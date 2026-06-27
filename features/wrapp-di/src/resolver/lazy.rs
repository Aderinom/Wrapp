use std::{
    any::type_name,
    fmt::Debug,
    ops::Deref,
    sync::{Arc, OnceLock},
};

use futures::{FutureExt, SinkExt, future::Shared};
use futures_channel::oneshot;

use crate::{
    errors::{InjectError, RequireError},
    initiator::{DiHandle, DiRequest, DiResponseReceiver},
    resolver::Resolver,
    types::{DependencyInfo, Injectable, Instance, TypeInfo},
};

/// Shared receiver for a lazily resolved instance.
///
/// [`Shared`] lets the single backing oneshot be observed from every clone, handling
/// the "resolve once, read many" synchronization so we don't have to.
type SharedInstance = Shared<DiResponseReceiver<Instance>>;

/// Lazily resolved dependency
///
/// Should only be accessed after the DI process has completed.
///
/// ### Panics
///
/// If accessed before DI process has completed
///
/// Note:
///
/// This Type by itself has many panic conditions - However if used in the DI context, no panics
/// should happen unless:
/// - It is accessed during the Injection Phase
/// - It is accessed after DI has already Failed
pub struct Lazy<T: Injectable> {
    /// Shared source future, resolved once the instance becomes available.
    source: SharedInstance,
    /// Caches the downcast result so [`get`](Self::get) can hand out a stable reference.
    resolved: OnceLock<Result<Arc<T>, InjectError>>,
}
impl<T: Injectable + Debug> Debug for Lazy<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Use `try_get` so debugging an unresolved `Lazy` does not panic.
        f.debug_tuple("Lazy").field(&self.try_get()).finish()
    }
}
impl<T: Injectable> Deref for Lazy<T> {
    type Target = Arc<T>;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}
impl<T: Injectable> Resolver for Lazy<T> {
    async fn resolve(handle: &mut DiHandle) -> Result<Self, InjectError>
    where
        Self: Sized,
    {
        let (tx, rx) = oneshot::channel();

        tracing::trace!(
            "Lazy dependency {} requested - will be resolved when available",
            type_name::<T>()
        );

        handle
            .request_sender
            .send(DiRequest::Require {
                type_info: TypeInfo::of::<T>(),
                response_channel: tx,
            })
            .await
            .map_err(|_| InjectError::HandleClosed)?;

        // We queue the request, but don't wait for the result here.
        Ok(Self {
            source: rx.shared(),
            resolved: OnceLock::new(),
        })
    }

    fn dependency_info() -> DependencyInfo {
        DependencyInfo {
            type_info: TypeInfo::of::<T>(),
            optional: false,
            lazy: true,
        }
    }
}

impl<T: Injectable> Lazy<T> {
    /// Accesses the lazy dependency.
    ///
    /// # Panics
    /// - When accessed before the DI container init has completed.
    /// - When accessed after DI has already failed.
    #[must_use]
    pub fn get(&self) -> &Arc<T> {
        self.try_get()
            .expect("Lazy inject accessed before initialized")
            .expect("Lazy inject result contained an error - this should not happen if accessed after DI has completed")
    }

    /// Tries to access the lazy dependency without blocking.
    ///
    /// Returns `None` while the dependency has not been resolved yet.
    pub fn try_get(&self) -> Option<Result<&Arc<T>, &InjectError>> {
        if self.resolved.get().is_none() {
            // Drive the shared future once with a no-op waker. This yields `None`
            // while the backing oneshot has not produced a value yet.
            let output = self.source.clone().now_or_never()?;
            // First writer wins; concurrent callers just observe the same result.
            let _ = self.resolved.set(Self::downcast(output));
        }

        self.resolved.get().map(Result::as_ref)
    }

    /// Resolves as soon as the lazy dependency is available.
    ///
    /// Must not be awaited during module construction - the dependency may not be
    /// constructed yet at that point.
    ///
    /// # Errors
    ///
    /// See [`InjectError`] for possible errors during resolution.
    ///
    /// # Panics
    /// - When accessed after DI has already failed.
    /// - When accessed before the DI container init has completed.
    // Note: Maybe add a second DI stage (Injection, Pre-Start) - where this is allowed.
    pub async fn wait(&self) -> Result<&Arc<T>, &InjectError> {
        // Await the shared future so a real waker is registered.
        let _ = self.source.clone().await;
        self.try_get()
            .expect("source resolved - try_get must return a value")
    }

    /// Downcasts the resolved instance into the requested type.
    fn downcast(
        output: Result<Result<Instance, RequireError>, oneshot::Canceled>,
    ) -> Result<Arc<T>, InjectError> {
        let instance = output.map_err(|_| InjectError::HandleClosed)??;
        instance.downcast::<T>().map_err(|actual_type| {
            RequireError::DowncastFailed {
                required_type: type_name::<T>(),
                actual_type,
            }
            .into()
        })
    }
}

pub struct LazyOption<T: Injectable> {
    lazy: Lazy<T>,
}
impl<T: Injectable + Debug> Debug for LazyOption<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.get() {
            Some(instance) => f.debug_tuple("LazyOption").field(instance).finish(),
            None => f.debug_tuple("LazyOption").field(&"None").finish(),
        }
    }
}
impl<T: Injectable> Resolver for LazyOption<T> {
    async fn resolve(handle: &mut DiHandle) -> Result<Self, InjectError>
    where
        Self: Sized,
    {
        Ok(Self {
            lazy: Lazy::<T>::resolve(handle).await?,
        })
    }

    fn dependency_info() -> DependencyInfo {
        DependencyInfo {
            type_info: TypeInfo::of::<T>(),
            optional: true,
            lazy: true,
        }
    }
}
impl<T: Injectable> LazyOption<T> {
    /// Accesses the Lazy Dependency
    ///
    /// # Panics
    /// - If accessed after DI has failed
    #[must_use]
    pub fn get(&self) -> Option<&Arc<T>> {
        match self.lazy.try_get() {
            None => None,
            Some(Ok(result)) => Some(result),
            Some(Err(err)) => {
                match err {
                    InjectError::RequireError(
                        RequireError::TypeDisabled(_) | RequireError::TypeMissing(_),
                    ) => None,
                    err => {
                        panic!("Accessed LazyOption after DI failure: {err:?}");
                    }
                }
            }
        }
    }
}
