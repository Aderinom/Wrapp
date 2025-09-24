use std::{
    any::type_name,
    fmt::Debug,
    future::Future,
    ops::Deref,
    sync::{Arc, Mutex, OnceLock},
    task::{Context, Poll},
};

use futures::{FutureExt, SinkExt};
use futures_channel::oneshot;
use pin_project_lite::pin_project;

use crate::{
    errors::{InjectError, RequireError},
    initiator::{DiHandle, DiRequest, DiResponseReceiver},
    resolver::Resolver,
    types::{DependencyInfo, Injectable, Instance, TypeInfo},
};

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
/// This Type by itself has many panic conditions - However if used in the DI context, no panics should happen unless:
/// - It is accessed during the Injection Phase
/// - It is accessed after DI has already Failed
///
pub struct Lazy<T: Injectable>(Arc<LazyInner<T>>);
impl<T: Injectable + Debug> Debug for Lazy<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Lazy").field(&self.get()).finish()
    }
}
struct LazyInner<T: Injectable> {
    once: OnceLock<Result<Arc<T>, InjectError>>,
    rx: Mutex<DiResponseReceiver<Instance>>,
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
        handle
            .request_sender
            .send(DiRequest::Require {
                type_info: TypeInfo::of::<T>(),
                response_channel: tx,
            })
            .await
            .map_err(|_| InjectError::HandleClosed)?;

        // Check if we got an immediate error

        Ok(Lazy(Arc::new(LazyInner {
            once: OnceLock::new(),
            rx: Mutex::new(rx),
        })))
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
    /// Accesses the Lazy Dependency
    ///
    /// # Panics
    /// - When accessed before the DI Container Init has completed
    pub fn get(&self) -> &Arc<T> {
        self.try_get()
            .expect("Lazy inject accessed before initialized")
            .as_ref()
            .expect("Lazy inject accessed after DI failed")
    }

    /// Try to access the lazy dependency
    pub fn try_get(&self) -> Option<Result<&Arc<T>, &InjectError>> {
        if let Some(result) = self.0.once.get() {
            return Some(result.as_ref());
        }
        
        // Lock receiver, so result is not taken out while we check
        let mut recv = self.0.rx.lock().unwrap();

        // Double check once - it might have been set while we waited for the lock
        if let Some(result) = self.0.once.get() {
            return Some(result.as_ref());
        }

        // Otherwise, try to receive result, and set once
        match recv.try_recv() {
            Ok(Some(rx)) => {
                let res = Lazy::<T>::downcast_recv(rx);
                self.0
                    .once
                    .set(res)
                    .map_err(|_| ())
                    .expect("holding lock on rx - this can't be set twice");

                    self.0.once.get().map(Result::as_ref)
            }
            Ok(None) => {
                None
            }
            Err(_) => {
                Some(Err(&InjectError::HandleClosed))
            }
        }
    }

    /// Resolves as soon as the lazy is available
    /// 
    /// Must not be waited on during module construction 
    // Note: Maybe Add a second DI stage (Injection, Pre Start) - where this is allowed
    pub fn wait_result(&self) -> LazyFuture<T> {
        LazyFuture { lazy: &self.0 }
    }
}
impl<T: Injectable> Lazy<T> {
    fn downcast_recv(recv: Result<Instance, RequireError>) -> Result<Arc<T>, InjectError> {
        match recv {
            Ok(instance) => instance.downcast().map_err(|e| {
                RequireError::DowncastFailed {
                    required_type: type_name::<T>(),
                    actual_type: e,
                }
                .into()
            }),
            Err(e) => Err(e.into()),
        }
    }
}

pin_project! {
    struct LazyFuture<'a, T:Injectable> {
        #[pin]
        lazy: &'a LazyInner<T>,
    }
}
impl<'a, T: Injectable> Future for LazyFuture<'a, T> {
    type Output = &'a Result<Arc<T>, InjectError>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> std::task::Poll<Self::Output> {
        // Lock receiver, so result is not taken out while we check
        let mut rx = self.lazy.rx.lock().unwrap();

        // Check if result is ready
        if let Some(result) = self.lazy.once.get() {
            return Poll::Ready(result);
        }

        // Poll recevier
        match rx.poll_unpin(cx) {
            Poll::Ready(recv) => {
                // We have a result, handle it and set once lock
                let res = match recv {
                    Ok(instance) => Lazy::<T>::downcast_recv(instance),
                    Err(e) => Err(e.into()),
                };

                self.lazy
                    .once
                    .set(res)
                    .map_err(|_| ())
                    .expect("holding lock on rx - this can't be set");

                Poll::Ready(self.lazy.once.get().expect("just set once - must be set"))
            }
            Poll::Pending => Poll::Pending,
        }
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
        Ok(LazyOption {
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
    /// Accesses the Lazy Dependency - returning an error on access
    pub fn try_get(&self) -> Option<Result<&Arc<T>, &InjectError>> {
        self.lazy.try_get()
    }

    /// Accesses the Lazy Dependency
    ///
    /// # Panics
    /// - If accessed after DI has failed
    pub fn get(&self) -> Option<&Arc<T>> {
        match self.lazy.try_get() {
            None => None,
            Some(Ok(result)) => return Some(result),
            Some(Err(err)) => match err {
                InjectError::RequireError(RequireError::TypeDisabled(_))
                | InjectError::RequireError(RequireError::TypeMissing(_)) => {
                    return None;
                }
                err => {
                    panic!("Accessed LazyOption after DI failure: {:?}", err);
                }
            },
        }
    }
}
