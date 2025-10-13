use std::{collections::HashMap, fmt::Debug, sync::Mutex};

use axum::serve::Listener;
use utoipa_axum::router::OpenApiRouter;

use crate::traits::{DynError, MakeAxumListener, ServeAxum};

pub mod builder;
pub mod traits;

// #####################################################
// Generic Strategy code

/// A Strategy is any component which can serve requests, be it HTTP, gRPC, Kafka, etc.
///
/// How the Strategy serves requests is up to the Strategy implementation.
trait Strategy {
    /// Tries to start serving the Strategy, will be called once during startup.
    ///
    /// If the Strategy fails to start, it should return an error.
    /// After the Strategy has started, run until the application is shut down.
    ///
    /// TODO: How to signal that serve was successful/strategy is running?
    async fn serve(self) -> Result<(), DynError>;
}

struct StrategyTags {
    tags: HashMap<String, String>,
    labels: HashMap<String, Vec<String>>,
}

// #####################################################
// Axum specific code

struct WrappAxumStrategy<MakeListenerFn, ServeFn>
where
    MakeListenerFn: MakeAxumListener,
    ServeFn: ServeAxum<MakeListenerFn::ListenerType>,
{
    listener_fn: MakeListenerFn,
    serve_function: ServeFn,
    base_router: Mutex<OpenApiRouter<()>>,
    tags: StrategyTags,
}

impl<A, B> WrappAxumStrategy<A, B>
where
    A: MakeAxumListener,
    B: ServeAxum<A::ListenerType>,
{
    /// Create a new Axum Strategy Builder
    pub fn builder() -> builder::WrappAxumStrategyBuilder<(), ()> {
        builder::WrappAxumStrategyBuilder::new()
    }
}

impl<MakeListenerFn, ServeFn> Strategy for WrappAxumStrategy<MakeListenerFn, ServeFn>
where
    MakeListenerFn: MakeAxumListener,
    ServeFn: ServeAxum<MakeListenerFn::ListenerType>,
{
    async fn serve(self) -> Result<(), DynError> {
        let listener = self.listener_fn.create_listener().await?;
        let router = self.base_router.into_inner()?;
        let (router, openapi) = router.split_for_parts();
        self.serve_function.serve(listener, router).await
    }
}
