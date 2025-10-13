use std::{collections::HashMap, fmt::Debug, sync::Mutex};

use axum::serve::Listener;
use utoipa_axum::router::OpenApiRouter;

use crate::{
    StrategyTags, WrappAxumStrategy,
    traits::{DynError, MakeAxumListener, ServeAxum},
};

pub struct WrappAxumStrategyBuilder<MakeListenerFn, ServeFn> {
    listener_fn: MakeListenerFn,
    serve_function: ServeFn,
    base_router: OpenApiRouter<()>,
    tags: StrategyTags,
}
// Initial builder state, no listener or serve function bound yet
impl WrappAxumStrategyBuilder<(), ()> {
    pub fn new() -> WrappAxumStrategyBuilder<(), ()> {
        WrappAxumStrategyBuilder {
            listener_fn: (),
            serve_function: (),
            base_router: OpenApiRouter::new(),
            tags: StrategyTags {
                tags: HashMap::new(),
                labels: HashMap::new(),
            },
        }
    }
}
// Only allow setting listener if no Serve function is set yet
impl<AnyListener> WrappAxumStrategyBuilder<AnyListener, ()> {
    #[cfg(feature = "tokio")]
    /// Will bind the Axum application to the given TCP address
    pub fn listener_tcp(
        self,
        addr: impl tokio::net::ToSocketAddrs + Send + 'static,
    ) -> WrappAxumStrategyBuilder<
        impl MakeAxumListener<ListenerType = tokio::net::TcpListener>,
        DefaultServeAxum<tokio::net::TcpListener>,
    > {
        self.custom_listener(move || async move { Ok(tokio::net::TcpListener::bind(addr).await?) })
    }

    /// Set the a custom function to create the listener for the Axum application
    /// This function will be called once during startup to create the listener.
    ///
    ///
    /// Example:
    /// ```no_run
    /// let strategy = WrappAxumStrategyBuilder::new()
    ///     .custom_listener(|| async {
    ///         Ok(tokio::net::TcpListener::bind("127.0.0.1:3000").await?)
    ///     });
    /// ```
    ///
    pub fn custom_listener<MakeListenerFn: MakeAxumListener>(
        self,
        listener_fn: MakeListenerFn,
    ) -> WrappAxumStrategyBuilder<MakeListenerFn, DefaultServeAxum<MakeListenerFn::ListenerType>>
    {
        WrappAxumStrategyBuilder {
            listener_fn,
            serve_function: DefaultServeAxum::new(),
            base_router: self.base_router,
            tags: self.tags,
        }
    }
}
// Only allow setting serve function if it accepts the correct listener type
impl<MakeListenerFn, AnyServe> WrappAxumStrategyBuilder<MakeListenerFn, AnyServe>
where
    MakeListenerFn: MakeAxumListener,
{
    /// Set the function which starts serving the Axum application
    pub fn custom_serve<ServeFn: ServeAxum<MakeListenerFn::ListenerType>>(
        self,
        serve_function: ServeFn,
    ) -> WrappAxumStrategyBuilder<MakeListenerFn, ServeFn> {
        WrappAxumStrategyBuilder {
            listener_fn: self.listener_fn,
            serve_function: serve_function,
            base_router: self.base_router,
            tags: self.tags,
        }
    }
}
// Only allow building if both listener and serve function are set
impl<MakeListenerFn, ServeFn> WrappAxumStrategyBuilder<MakeListenerFn, ServeFn>
where
    MakeListenerFn: MakeAxumListener,
    ServeFn: ServeAxum<MakeListenerFn::ListenerType>,
{
    pub fn build(self) -> WrappAxumStrategy<MakeListenerFn, ServeFn> {
        WrappAxumStrategy {
            listener_fn: self.listener_fn,
            serve_function: self.serve_function,
            base_router: Mutex::new(self.base_router),
            tags: self.tags,
        }
    }
}

/// Default implementation of ServeAxum, uses axum::serve to serve the application
pub struct DefaultServeAxum<L>
where
    L: Listener + Send + 'static,
    L::Addr: Debug,
{
    _marker: std::marker::PhantomData<L>,
}
impl<L> DefaultServeAxum<L>
where
    L: Listener + Send + 'static,
    L::Addr: Debug,
{
    fn new() -> Self {
        DefaultServeAxum {
            _marker: std::marker::PhantomData,
        }
    }
}
impl<L> ServeAxum<L> for DefaultServeAxum<L>
where
    L: Listener + Send + 'static,
    L::Addr: Debug,
{
    fn serve(
        self,
        listener: L,
        router: axum::Router,
    ) -> impl Future<Output = Result<(), DynError>> {
        async move {
            axum::serve(listener, router.into_make_service()).await?;
            Ok(())
        }
    }
}
