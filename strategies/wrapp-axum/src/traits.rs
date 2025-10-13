use std::fmt::Debug;

use axum::serve::Listener;
use utoipa::openapi::OpenApi;

/// A boxed error type for convenience
pub type DynError = Box<dyn std::error::Error + Send + Sync>;

/// Creates a Listener asynchronously, will only be called once during startup
///
/// Is implemented for any async function that returns a [Result<Listener, Box<dyn Error + Send + Sync>>]

pub trait MakeAxumListener
where
    <Self::ListenerType as Listener>::Addr: std::fmt::Debug,
{
    type ListenerType: Listener + Send + 'static;

    fn create_listener(self) -> impl Future<Output = Result<Self::ListenerType, DynError>> + Send;
}
/// Any Async function that returns a Listener can be used as a MakeAxumListener
impl<Listen, Fun, FunFuture> MakeAxumListener for Fun
where
    Listen: Listener + Send + 'static,
    Listen::Addr: Debug, // Required by axum::serve
    Fun: FnOnce() -> FunFuture,
    FunFuture: Future<Output = Result<Listen, DynError>> + Send,
{
    type ListenerType = Listen;

    #[allow(refining_impl_trait)]
    fn create_listener(self) -> FunFuture {
        self()
    }
}

/// Starts serving the Axum application, will be called once during startup
///
/// Is implemented for any async function that returns a [Result<Listener, Box<dyn Error + Send + Sync>>]
pub trait ServeAxum<L: Listener + Send + 'static> {
    fn serve(self, listener: L, router: axum::Router)
    -> impl Future<Output = Result<(), DynError>>;
}
/// Any Async function, taking a listener can be used as a Serve function
impl<L, Fun, FunFuture> ServeAxum<L> for Fun
where
    L: Listener + Send + 'static,
    Fun: FnOnce(L, axum::Router) -> FunFuture,
    FunFuture: Future<Output = Result<(), DynError>> + Send,
{
    #[allow(refining_impl_trait)]
    fn serve(self, listener: L, router: axum::Router) -> FunFuture {
        self(listener, router)
    }
}
