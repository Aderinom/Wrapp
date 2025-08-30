//! Wrapp DI consists of following components:
//!
//! 1. DiBuilder - for registering all factories and instances.
//! 2. DiInitiator - which executes all factories, and handles injection requests during their initialization.
//! 2.1 Injection Handles - which are valid during the AppInit Phase, and allow a component to request its dependencies.
//! 3. DIContainer - which is the final container that holds all created instances.
//!
//! General logic:
//! AppBuilder calls BUild
//!
//! AppGraph queries all factories for their dependencies and creates a graph.
//! AppGraph checks the graph for circular dependencies, and aborts DI if any are found.
//!
//! AppInitiator creates tasks for all factories.
//! Factories start asking for their dependencies.
//! Some Factory will not require a dependency (or just requires already existing instances), and as such will complete
//! After Completion, AppInitiator gets the result of the completed task.
//! AppInitiator then informs all factories which were waiting for this dependency.
//! Step by step all factories will complete
//! After All factories have completed, AppInitiator creates the final DIContainer.
//! AppInitiator publishes the DIcontainer through a channel to all dependencies.
//! DiContainer wil also be given to the caller of AppBuilder.build()
//!

use std::{
    any::{type_name, Any, TypeId},
    collections::{HashMap, HashSet},
    convert::Infallible,
    error::{self, Error},
    fmt::Debug,
    future::Future,
    pin::pin,
    sync::Arc,
    task::Context,
    thread::{self, sleep},
    time::Duration,
};

use futures::{stream::FuturesUnordered, SinkExt, StreamExt};
use futures_channel::{mpsc, oneshot};
use thiserror::Error;

/// All errors must be clone
pub trait CloneError: std::error::Error + Clone {}
impl<T: std::error::Error + Clone> CloneError for T {}

type DynError = Box<dyn std::error::Error>;

/// Any Type which is Send 'static - used for where bounds
pub trait StaticSend: Send + 'static {}
impl<T: Send + 'static> StaticSend for T {}

/// WE assume that we are using a multithreaded async runtime
/// So anything injectable needs to be Send + Sync + 'static
pub trait Injectable: Send + Sync + 'static {}
impl<T: Send + Sync + 'static> Injectable for T {}

/// Instance of a Provider
#[derive(Clone)]
pub struct Instance {
    pub type_name: &'static str,
    pub instance: Arc<dyn Any + Send + Sync + 'static>,
}
impl Instance {
    fn new<ExistingInstance: Injectable>(instance: ExistingInstance) -> Self {
        Instance {
            type_name: std::any::type_name::<ExistingInstance>(),
            instance: Arc::new(instance),
        }
    }

    fn downcast<T: Injectable>(&self) -> Result<Arc<T>, &'static str> {
        match Arc::downcast::<T>(self.instance.clone()) {
            Ok(downcasted) => Ok(downcasted),
            Err(_) => Err(self.type_name),
        }
    }
}

pub struct DependencyInfo {
    pub type_name: &'static str,
    pub type_id: TypeId,
    pub optional: bool,
    pub lazy: bool,
}

#[derive(Hash, PartialEq, Eq)]
pub struct TypeInfo {
    pub type_name: &'static str,
    pub type_id: TypeId,
}
impl TypeInfo {
    fn of<T: 'static + ?Sized>() -> TypeInfo {
        TypeInfo {
            type_name: std::any::type_name::<T>(),
            type_id: TypeId::of::<T>(),
        }
    }
}

/// A Factory providing instances of a given type
pub trait InstanceFactory {
    type Provides: Injectable;

    /// Returns the typeinfo about the factory's provided type
    fn supplies() -> TypeInfo {
        TypeInfo::of::<Self::Provides>()
    }

    /// Returns a list of dependencies the factory requires to supply it's type
    fn get_dependencies() -> Vec<DependencyInfo>;

    /// Constructs a new instance of the factory's provided type
    ///
    /// Returns the constructed instance, or an error if either Dependencies are not satisfied or the Instantiation failed
    #[allow(async_fn_in_trait)]
    async fn construct(&mut self, di: DiHandle) -> Result<Self::Provides, DynError>;

    /// Returns an indicator to whether the factory is enabled or not
    #[allow(async_fn_in_trait)]
    async fn is_enabled(&mut self, di: DiHandle) -> Result<bool, impl Into<DynError>> {
        let _ = di;
        Ok::<_, Infallible>(true)
    }
}

/// Wrapper Trait for factories, providing instances of Any
pub trait DynFactory {
    fn supplies(&self) -> TypeInfo;

    /// Returns a list of dependencies for the factory
    fn get_dependencies(&self) -> Vec<DependencyInfo>;

    /// Constructs a new instance of the factory's provided type, fulfilling all its dependencies
    fn construct(
        &mut self,
        di: DiHandle,
    ) -> Box<dyn Future<Output = Result<Instance, DynError>> + '_>;

    /// Returns a boolean indicating whether the factory is enabled or not
    fn is_enabled(
        &mut self,
        di: DiHandle,
    ) -> Box<dyn Future<Output = Result<bool, DynError>> + '_> {
        di; // Ignore unused
        Box::new(async { Ok(true) })
    }
}
// Impl DynFactory for any InstanceFactory
impl<T: Injectable, SpecificFactory: InstanceFactory<Provides = T>> DynFactory for SpecificFactory {
    fn supplies(&self) -> TypeInfo {
        SpecificFactory::supplies()
    }

    fn get_dependencies(&self) -> Vec<DependencyInfo> {
        SpecificFactory::get_dependencies()
    }

    fn construct(
        &mut self,
        di: DiHandle,
    ) -> Box<dyn Future<Output = Result<Instance, DynError>> + '_> {
        let construction_fut = async {
            // Forward the call to the specific implementation
            SpecificFactory::construct(self, di)
                .await
                .map(Instance::new)
                .map_err(|e| e.into())
        };

        Box::new(construction_fut)
    }

    fn is_enabled(
        &mut self,
        di: DiHandle,
    ) -> Box<dyn Future<Output = Result<bool, DynError>> + '_> {
        let future = async {
            // Forward the call to the specific implementation
            SpecificFactory::is_enabled(self, di)
                .await
                .map_err(|e| e.into())
        };

        Box::new(future)
    }
}

/// Graph of the entire application
/// Used to check circular dependencies and enables visualization of APP
struct DependencyGraph {
    //TODO: Implement
}

///
/// The DI Consist of three Parts.
/// 1. The AppBuilder where one registers all factories and instances
/// 2. Then for initialization
///
///
///
pub struct DiBuilder {
    /// Registered factories which can provide instances
    registered_factories: Vec<Box<dyn DynFactory>>,
    /// Registered already created instances
    registered_instances: HashMap<TypeInfo, Instance>,
}
impl DiBuilder {
    pub fn new() -> Self {
        DiBuilder {
            registered_factories: Vec::new(),
            registered_instances: HashMap::new(),
        }
    }
}
impl DiBuilder {
    pub fn add_instance<T: Injectable>(mut self, instance: T) -> Self {
        self.registered_instances
            .insert(TypeInfo::of::<T>(), Instance::new(instance));
        self
    }

    pub fn add_factory<Factory: InstanceFactory + 'static>(mut self, factory: Factory) -> Self {
        self.registered_factories.push(Box::new(factory));
        self
    }

    pub async fn build(self) -> Result<DiContainer, InitError> {
        DiInitiator::new().initiate(self, None).await
    }

    pub async fn build_timeout(self, timeout: Duration) -> Result<DiContainer, InitError> {
        DiInitiator::new().initiate(self, Some(timeout)).await
    }
}

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

/// Allows custom behaviour on injection
pub trait ResolveStrategy {
    #[allow(async_fn_in_trait)]
    async fn resolve(handle: &mut DiHandle) -> Result<Self, InjectError>
    where
        Self: Sized;
}

impl<T: Injectable> ResolveStrategy for Arc<T> {
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
}

/// DI Handle for resolving dependencies and getting instances from the registry.
/// The DI Handle is only valid during instanciation of the Application.
/// Afterwards the DI Container can be used directly for dependency injection.
#[derive(Clone)]
pub struct DiHandle {
    request_sender: mpsc::Sender<DiRequest>,
}

impl DiHandle {
    pub async fn resolve<T: Injectable + ResolveStrategy>(&mut self) -> Result<T, InjectError> {
        T::resolve(self).await
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

pub type DiResponseSender<For> = oneshot::Sender<Result<For, RequireError>>;
pub type DiResponseReceiver<For> = oneshot::Receiver<Result<For, RequireError>>;

pub enum DiRequest {
    // Require an instance of a specific type
    Require {
        type_info: TypeInfo,
        response_channel: DiResponseSender<Instance>,
    },
    // Requires a reference to the Application once it has been build
    RequireApp {
        response_channel: DiResponseSender<DiContainer>,
    },
}

struct DiInitiator {
    request_rx: mpsc::Receiver<DiRequest>,
    request_tx: mpsc::Sender<DiRequest>,

    all_registered_type_ids: HashSet<TypeId>,

    /// Waiters for instance results
    instance_waiters: HashMap<TypeId, Vec<DiResponseSender<Instance>>>,
    /// Waiters for the final DiContainer
    container_waiters: Vec<DiResponseSender<DiContainer>>,

    /// All produced instances - None = Type is known but disabled
    instances: HashMap<TypeId, (TypeInfo, Option<Instance>)>,
}

impl DiInitiator {
    fn new() -> DiInitiator {
        let (injection_request_sender, injection_request_receiver) = mpsc::channel(10);
        DiInitiator {
            request_rx: injection_request_receiver,
            request_tx: injection_request_sender,
            all_registered_type_ids: HashSet::new(),
            instances: HashMap::new(),
            instance_waiters: HashMap::new(),
            container_waiters: Vec::new(),
        }
    }

    pub async fn initiate(
        mut self,
        blueprint: DiBuilder,
        timeout: Option<Duration>,
    ) -> Result<DiContainer, InitError> {
        // If we have a timeout - spawn a thread to signal once it's done
        let (timeout_tx, timeout_rx) = oneshot::channel::<()>();
        if let Some(timeout) = timeout {
            // We don't join the thread - it will just die after the timeout
            thread::spawn(move || {
                sleep(timeout);
                let _ = timeout_tx.send(());
            });
        };

        // Start initiating all Instances
        if let Err(e) = self.try_initiate(blueprint, timeout_rx).await {
            // On fail - inform all waiters
            let msg = Err(e.clone().into());
            for (_, waiters) in self.instance_waiters {
                for waiter in waiters.into_iter() {
                    let _ = waiter.send(msg.clone());
                }
            }

            let msg = Err(e.clone().into());
            for waiter in self.container_waiters {
                let _ = waiter.send(msg.clone());
            }

            return Err(e);
        }

        tracing::debug!("All");

        // TODO: Send App instance to all waiting requests
        let container = DiContainer::new(self.instances);

        for waiter in self.container_waiters {
            let _ = waiter.send(Ok(container.clone()));
        }

        Ok(container)
    }

    /// Starts all registered factories and waits for them to complete
    async fn try_initiate(
        &mut self,
        blueprint: DiBuilder,
        mut timeout: oneshot::Receiver<()>,
    ) -> Result<(), InitError> {
        let DiBuilder {
            registered_factories,
            registered_instances,
        } = blueprint;

        tracing::debug!(
            "Initializing application with {} factories and {} instances",
            registered_factories.len(),
            registered_instances.len()
        );
        //TODO: Check for circular dependencies

        // Add all pre build instances to the results

        for (info, instance) in registered_instances.into_iter() {
            self.all_registered_type_ids.insert(info.type_id);
            self.instances.insert(info.type_id, (info, Some(instance)));
        }

        // ###############################################
        // Begin instantiation of all factories
        let mut factory_futures = FuturesUnordered::new();
        for mut factory in registered_factories {
            self.all_registered_type_ids
                .insert(factory.supplies().type_id);
            let handle = self.get_handle();

            // Returns error if DI failed
            // Returns None if dependency is disabled
            let factory_future = async move {
                let supply_info = factory.supplies();
                let result = async {
                    // Check if the factory is enabled
                    match Box::into_pin(factory.is_enabled(handle.clone())).await? {
                        true => {
                            tracing::debug!("Factory for {} is enabled", supply_info.type_name)
                        }
                        false => {
                            tracing::debug!("Factory for {} is disabled", supply_info.type_name);
                            return Ok(None);
                        }
                    }

                    // Construct factory
                    let instance = Box::into_pin(factory.construct(handle.clone())).await?;

                    tracing::debug!("Constructed instance of {}", instance.type_name);
                    Ok::<_, DynError>(Some(instance))
                }
                .await;

                (supply_info, result)
            };

            factory_futures.push(factory_future);
        }

        // Start handling injection requests and wait for all factories to finish
        // let pin_set = pin!(set);
        let factory_count = factory_futures.len();

        loop {
            let factories_left = factory_futures.len();
            tracing::debug!(
                "Waiting for factories to finish [{factories_left} of {factory_count} complete]"
            );

            futures::select! {
                request = self.request_rx.select_next_some() => {
                    self.handle_injection_request(request);
                }
                result = factory_futures.next() => {
                    if self.handle_factory_result(result)? {
                        break;
                    }
                }
                _ = timeout => {
                    return Err(InitError::Timeout)
                }
            }
        }

        Ok(())
    }

    /// Handle the result of a factory future
    ///
    /// Returns true if complete
    fn handle_factory_result(
        &mut self,
        result: Option<(TypeInfo, Result<Option<Instance>, Box<dyn Error>>)>,
    ) -> Result<bool, InitError> {
        let (info, result) = match result {
            Some(result) => result,
            None => {
                // If no more tasks are left, exit the loop
                // all injection requests must now also be handled as nothing is left to be build
                debug_assert!(
                    self.instance_waiters.is_empty(),
                    "Not all waiters were satisfied"
                );
                return Ok(true);
            }
        };

        match result {
            Ok(instance) => {
                handle_created_instance(self, info, instance);
            }
            Err(err) => {
                // If one factory fails - abort DI
                return Err(InitError::FactoryFailed {
                    product: info.type_name,
                    error: Arc::new(err),
                });
            }
        };

        return Ok(false);

        fn handle_created_instance(
            this: &mut DiInitiator,
            info: TypeInfo,
            instance: Option<Instance>,
        ) -> () {
            let type_id = info.type_id;
            let type_name = info.type_name;
            // Add instance to results
            this.instances.insert(type_id, (info, instance.clone()));

            let message = match instance {
                Some(instance) => Ok(instance),
                None => Err(RequireError::TypeDisabled(type_name)),
            };

            // Inform all waiters with a result
            for waiter in this
                .instance_waiters
                .remove(&type_id)
                .into_iter()
                .flat_map(Vec::into_iter)
            {
                let _ = waiter.send(message.clone());
            }
        }
    }

    /// Get a handle to the DiInitiator
    ///
    /// The handle is only valid before and during Initiation.
    pub fn get_handle(&self) -> DiHandle {
        DiHandle {
            request_sender: self.request_tx.clone(),
        }
    }
}

// Injection Request handlers
impl DiInitiator {
    fn handle_injection_request(&mut self, request: DiRequest) {
        match request {
            DiRequest::Require {
                type_info,
                response_channel,
            } => {
                self.handle_instance_require(type_info, response_channel);
            }
            DiRequest::RequireApp { response_channel } => self.handle_app_require(response_channel),
        }
    }

    fn handle_instance_require(
        &mut self,
        info: TypeInfo,
        response_channel: DiResponseSender<Instance>,
    ) {
        // Check if the TypeId is registered
        if !self.all_registered_type_ids.contains(&info.type_id) {
            tracing::error!("Tried to require an unregistered type: {}", info.type_name);
            let _ = response_channel.send(Err(RequireError::TypeMissing(info.type_name)));
            return;
        }

        // Check if we already have a result
        match self.instances.get(&info.type_id) {
            Some((_, result)) => {
                let _ = match result {
                    Some(instance) => response_channel.send(Ok(instance.clone())),
                    None => response_channel.send(Err(RequireError::TypeDisabled(info.type_name))),
                };
                return;
            }
            None => {} // No result yet
        }

        // Otherwise add the request to the waiters list
        self.instance_waiters
            .entry(info.type_id)
            .or_default()
            .push(response_channel);
    }

    fn handle_app_require(&mut self, response_channel: DiResponseSender<DiContainer>) {
        self.container_waiters.push(response_channel);
    }
}

/// Container holding all initiated instances
#[derive(Clone)]
pub struct DiContainer(pub Arc<DiContainerInner>);
pub struct DiContainerInner {
    instances: HashMap<TypeId, (TypeInfo, Option<Instance>)>,
}
impl Debug for DiContainer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut map = f.debug_struct("DiContainer");
        for (info, instance) in self.0.instances.values() {
            let val = if instance.is_some() {
                "enabled"
            } else {
                "disabled"
            };
            map.field(&info.type_name, &val);
        }
        map.finish()
    }
}

impl DiContainer {
    fn new(instances: HashMap<TypeId, (TypeInfo, Option<Instance>)>) -> Self {
        Self(Arc::new(DiContainerInner { instances }))
    }

    /// Attempts to get the requested type
    pub fn require<T: Injectable>(&self) -> Result<Arc<T>, RequireError> {
        match self.0.instances.get(&TypeId::of::<T>()) {
            Some((_, Some(instance))) => {
                instance
                    .downcast()
                    .map_err(|actual_type| RequireError::DowncastFailed {
                        required_type: type_name::<T>(),
                        actual_type,
                    })
            }
            Some((_, None)) => Err(RequireError::TypeDisabled(type_name::<T>())),
            None => Err(RequireError::TypeMissing(type_name::<T>())),
        }
    }
}

//////////////////////////////////////////////////////////////////////

//ToThink
// TODO: Probably should have DI be a different thing as APP - let App contain DiContainer
