use std::{any::type_name, sync::Arc};

use futures::SinkExt;
use futures_channel::oneshot;

use crate::{
    errors::{InjectError, RequireError},
    initiator::{DiHandle, DiRequest},
    resolver::Resolver,
    types::{DependencyInfo, Injectable, TypeInfo},
};

impl<T: Injectable> Resolver for Arc<T> {
    async fn resolve(handle: &mut DiHandle) -> Result<Self, InjectError> {
        let (tx, rx) = oneshot::channel();
        handle
            .request_sender
            .send(DiRequest::Require {
                type_info: TypeInfo::of::<T>(),
                response_channel: tx,
            })
            .await?;

        let resolved = rx.await??;
        let downcasted = resolved
            .downcast::<T>()
            .map_err(|e| RequireError::DowncastFailed {
                required_type: type_name::<T>(),
                actual_type: e,
            })?;

        Ok(downcasted)
    }

    fn dependency_info() -> DependencyInfo {
        DependencyInfo {
            type_info: TypeInfo::of::<T>(),
            optional: false,
            lazy: false,
        }
    }
}

impl<Resolvable: Resolver> Resolver for Option<Resolvable> {
    async fn resolve(handle: &mut DiHandle) -> Result<Self, InjectError>
    where
        Self: Sized,
    {
        match Resolvable::resolve(handle).await {
            Ok(resolved) => Ok(Some(resolved)),
            Err(e) => match e {
                // If the required type is disabled, or not registered Option does not fail
                InjectError::RequireError(RequireError::TypeDisabled(_))
                | InjectError::RequireError(RequireError::TypeMissing(_)) => Ok(None),
                _ => Err(e),
            },
        }
    }

    fn dependency_info() -> DependencyInfo {
        let original = Resolvable::dependency_info();
        DependencyInfo {
            optional: true,
            ..original
        }
    }
}
