use std::sync::Arc;

use futures_channel::{mpsc, oneshot};
use thiserror::Error;

use crate::{dependency_graph::DependencyGraphErrors, types::DynError};

#[derive(Error, Debug)]
pub enum InjectError {
    /// Could not require the type
    #[error(transparent)]
    RequireError(#[from] RequireError),
    /// Injection Handle channel was closed
    #[error("Handle was closed, did you try using it after Initialization?")]
    HandleClosed,
    /// Generic error during Injection
    #[error("Error during injection: {0}")]
    Other(DynError),
}
impl From<mpsc::SendError> for InjectError {
    fn from(_: mpsc::SendError) -> Self {
        Self::HandleClosed
    }
}
impl From<oneshot::Canceled> for InjectError {
    fn from(_: oneshot::Canceled) -> Self {
        Self::HandleClosed
    }
}

/// Errors when trying to require a certain type
#[derive(thiserror::Error, Debug, Clone)]
pub enum RequireError {
    /// The required type is not known
    #[error("The required type is not known.")]
    TypeMissing(&'static str),
    /// The required type is disabled
    #[error("The required type is disabled.")]
    TypeDisabled(&'static str),
    /// Di container failed to initiate
    #[error(transparent)]
    InitError(#[from] InitError),

    #[error("Failed to downcast, required: '{required_type}' actual: '{actual_type}'")]
    DowncastFailed {
        required_type: &'static str,
        actual_type: &'static str,
    },
}

/// Errors while Initiating types
#[derive(thiserror::Error, Debug, Clone)]
pub enum InitError {
    /// There are issues with the dependency graph
    #[error(transparent)]
    DependencyGraphError(#[from] DependencyGraphErrors),

    /// A Factory failed to build
    #[error("Factory for '{product}' failed - error: {error:?}")]
    FactoryFailed {
        product: &'static str,
        error: Arc<DynError>,
    },
    /// Initiation timed out
    #[error("Initiation timed out")]
    Timeout,
}
