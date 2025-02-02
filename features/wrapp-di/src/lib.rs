use std::{
    any::{Any, TypeId}, collections::{HashMap, HashSet}, convert::Infallible, future::Future, pin::pin, sync::Arc
};

use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use futures_channel::{mpsc, oneshot};

type DynError = Box<dyn std::error::Error>;

/// Any Type which is Send 'static - used for where bounds
pub trait StaticSend: Send + 'static {}
impl<T: Send + 'static> StaticSend for T {}

/// Only types which can
/// 1. Be send to other threads (assuming multithreaded async runtime)
/// 2. Are Sync safe
/// 3. Have a static lifetime
/// Can be used for dependency injection in async Rust.
pub trait Injectable: Send + Sync + 'static {}
impl<T: Send + Sync + 'static> Injectable for T {}

// An Instance of a Provider
struct Instance {
    pub type_name: &'static str,
    pub instance: Arc<Box<dyn Any>>,
}

impl Instance {
    fn new<ExistingInstance: 'static>(instance: ExistingInstance) -> Self {
        Instance {
            type_name: std::any::type_name::<ExistingInstance>(),
            instance: Arc::new(Box::new(instance)),
        }
    }
}

struct DependencyInfo {
    pub type_name: &'static str,
    pub type_id: TypeId,
    pub optional: bool,
    pub lazy: bool,
}

struct TypeInfo {
    pub type_name: &'static str,
    pub type_id: TypeId,
}

/// A factory with no state
trait InstanceFactory {
    type Provides: 'static;

    fn supplies() -> TypeInfo {
        TypeInfo {
            type_name: std::any::type_name::<Self::Provides>(),
            type_id: TypeId::of::<Self::Provides>(),
        }
    }
    /// Returns a list of dependencies for the factory
    fn get_dependencies() -> Vec<DependencyInfo>;
    /// Constructs a new instance of the factory's provided type, fulfilling all its dependencies
    async fn construct(
        &mut self,
        di: &InjectionHandle,
    ) -> Result<Self::Provides, impl Into<DynError>>;
    /// Returns a boolean indicating whether the factory is enabled or not
    async fn is_enabled(&mut self, di: &InjectionHandle) -> Result<bool, impl Into<DynError>> {
        di;
        Ok::<_, Infallible>(true)
    }
}

/// Wrapper type for factories, allowing for dynamic dispatch
trait DynFactory {
    fn supplies(&self) -> TypeInfo;
    /// Returns a list of dependencies for the factory
    fn get_dependencies(&self) -> Vec<DependencyInfo>;
    /// Constructs a new instance of the factory's provided type, fulfilling all its dependencies
    fn construct(
        &mut self,
        di:  &InjectionHandle,
    ) -> Box<dyn Future<Output = Result<Instance, DynError>>>;
    /// Returns a boolean indicating whether the factory is enabled or not
    fn is_enabled(
        &mut self,
        di: &InjectionHandle,
    ) -> Box<dyn Future<Output = Result<bool, DynError>>> {
        di;
        Box::new(async { Ok(true) })
    }
}
impl<T: 'static, SpecificFactory: InstanceFactory<Provides = T>> DynFactory for SpecificFactory {
    fn supplies(&self) -> TypeInfo {
        TypeInfo {
            type_name: std::any::type_name::<T>(),
            type_id: TypeId::of::<T>(),
        }
    }

    fn get_dependencies(&self) -> Vec<DependencyInfo> {
        // Forward the call to the specific implementation
        SpecificFactory::get_dependencies()
    }

    fn construct(
        &mut self,
        di:  &InjectionHandle,
    ) -> Box<dyn Future<Output = Result<Instance, DynError>>> {
        let future = async {
            // Forward the call to the specific implementation
            SpecificFactory::construct(self, di)
                .await
                .map(Instance::new)
                .map_err(|e| e.into())
        };

        Box::new(future)
    }

    fn is_enabled(
        &mut self,
        di:  &InjectionHandle,
    ) -> Box<dyn Future<Output = Result<bool, DynError>>> {
        let future = async {
            // Forward the call to the specific implementation
            SpecificFactory::is_enabled(self, di)
                .await
                .map_err(|e| e.into())
        };

        Box::new(future)
    }
}

struct AppGraph {
    //TODO: Implement
}

struct AppBuilder {
    /// Registered factories which can provide instances
    registered_factories: Vec<Box<dyn DynFactory>>,
    /// Registered already created instances
    registered_instances: HashMap<TypeId, Instance>,
}
impl AppBuilder {
    pub fn new() -> Self {
        AppBuilder {
            registered_factories: Vec::new(),
            registered_instances: HashMap::new(),
        }
    }
}

impl AppBuilder {
    pub fn add_instance<T: 'static>(&mut self, instance: T) {
        self.registered_instances
            .insert(TypeId::of::<T>(), Instance::new(instance));
    }

    pub fn add_factory<Factory: InstanceFactory + 'static>(&mut self, factory: Factory) {
        self.registered_factories.push(Box::new(factory));
    }


    pub async fn build(self) -> Result<App, DiError> {
        let res = AppInitiator::new().initiate(self).await;
        Ok(())
    }
}

trait InjectionHandleStrategy {
    type Outputs;
    fn resolve(handle: &InjectionHandle) -> Self::Outputs;
}

/// DI Handle for resolving dependencies and getting instances from the registry.
/// The DI Handle is only valid during instanciation of the Application.
/// Afterwards the DI Container can be used directly for dependency injection.
struct InjectionHandle {
    injection_request_sender: mpsc::Sender<InjectionRequests>,
}

// impl DiHandle {
//     async fn resolve<T>(&self, type_name: &str) -> Result<Instance, DiHandleError> {

//     }

//     async fn resolve_lazy<T>(&self, type_name: &str) -> Result<Instance, DiHandleError> {

//     }

//     async fn resolve_using_strategy<Strategy: DiHandleStrategy>(&self) -> Result<Strategy::Outputs, DiHandleError> {
//         Strategy::resolve(self)
//     }
// }

#[derive(thiserror::Error, Debug)]
enum InjectError {
    #[error("The requested type is not registered.")]
    RequestedTypeMissing,
    #[error("The requested type is registered, but is not enabled.")]
    RequestedTypeDisabled,
    #[error("The Injection process was aborted due to an error.")]
    Aborted,
    #[error(transparent)]
    DiError(#[from] DiError),
}

#[derive(thiserror::Error, Debug)]
enum DiError {
    #[error("Factory for '{type_name}' failed - error: {error:?}")]
    FactoryFailed{
        type_name: &'static str,
        error: DynError,
    },
}





type InjectionResponseSender<For> = futures_channel::oneshot::Sender<Result<Arc<For>, InjectError>>;
type InjectionResponseReceiver<For> = futures_channel::oneshot::Receiver<Result<Arc<For>, InjectError>>;

enum InjectionRequests {
    // Require an instance of a specific type
    Require {
        wants_type_id: TypeId,
        response_channel: InjectionResponseSender<Instance>,
    },
    // Requires a reference to the Application once it has been build
    RequireApp {
        response_channel: InjectionResponseSender<App>,
    },
}

struct AppInitiator {
    injection_request_receiver: mpsc::Receiver<InjectionRequests>,
    injection_request_sender: mpsc::Sender<InjectionRequests>,

    all_registerd_type_ids: HashSet<TypeId>,
    ready_instances: HashMap<TypeId, Option<Arc<Instance>>>,

    instance_waiters: HashMap<TypeId, Vec<InjectionResponseSender<Instance>>>,
    app_waiters: Vec<InjectionResponseSender<App>>,
}

impl AppInitiator {
    fn new() -> AppInitiator {
        let (injection_request_sender, injection_request_receiver) = mpsc::channel(10);
        AppInitiator {
            injection_request_receiver,
            injection_request_sender,
            all_registerd_type_ids: HashSet::new(),
            ready_instances: HashMap::new(),
            instance_waiters: HashMap::new(),
            app_waiters: Vec::new(),
        }
    }

    pub async fn initiate(&mut self, blueprint: AppBuilder) -> Result<(), DiError> {
        let AppBuilder {
            registered_factories,
            registered_instances,
        } = blueprint;

        tracing::debug!("Initializing application with {} factories and {} instances", registered_factories.len(), registered_instances.len());
        //TODO: Check for circular dependencies

        // Begin instantiation of all factories
        let mut factory_futures = FuturesUnordered::new();
        for mut factory in registered_factories {
            let di = self.get_handle();

            let factory_future = async move {
                let supply_info = factory.supplies();
                // Check if the factory is enabled
                match Box::into_pin(factory.is_enabled(&di)).await? {
                    true => tracing::debug!("Factory for {} is enabled", supply_info.type_name),
                    false => {
                        tracing::debug!("Factory for {} is disabled", supply_info.type_name);
                        return Ok(None);
                    },
                }
                                
                // Construct factory
                let instance = Box::into_pin(factory.construct(&di)).await?;

                tracing::debug!("Constructed instance of {}", instance.type_name);
                Ok::<_, DynError>(Some(instance))
            };

            factory_futures.push(factory_future);
        }

        // Start handling injection requests and wait for all factories to finish
        // let pin_set = pin!(set);
        tracing::debug!("Waiting for factories to finish...");
        loop {
            futures::select! {
                request = self.injection_request_receiver.select_next_some() => {
                    self.handle_injection_request(request);
                }
                factory_result = factory_futures.next() => {
                    match factory_result {
                        Some(result) => self.handle_task_result(result),
                        None => break, // If no more tasks are left, exit the loop - all injection requests must now also be handled as nothing is left to be build
                    }
                }
            }
        }
        tracing::debug!("All factories have finished - App DI completed");
        // TODO: Send App instance to all waiting requests


        Ok(())
    }


    fn handle_task_result(&mut self, task_result:Result<Option<Instance>, DynError>) -> Result<(), DiError> {
        let task_result = match task_result {
            Ok(res)=>res,
            Err(e) => return Err(DiError::FactoryFailed{
                error: e,
                type_name: "Unknown", //TODO
            }),
        };

        self.ready_instances.insert(task_result.type_id(), Some(Arc::new(task_result)));
        

        // Add instance to results
        // Inform all waiters with a clone of the instance

        Ok(())
    }


    fn get_handle(&self) -> InjectionHandle {
        InjectionHandle {
            injection_request_sender: self.injection_request_sender.clone(),
        }
    }

}

// Injection Request handlers
impl AppInitiator {

    fn handle_injection_request(&mut self, request: InjectionRequests) {
        match request {
            InjectionRequests::Require { wants_type_id, response_channel } => {
                self.handle_instance_require(wants_type_id, response_channel);
            },
            InjectionRequests::RequireApp { response_channel } => 
            self.handle_app_require(response_channel),
        }
    }



    fn handle_instance_require(&mut self, type_id: TypeId, response_channel: InjectionResponseSender<Instance>) {
        // Check if the TypeId is registered
        if !self.all_registerd_type_ids.contains(&type_id) {
            tracing::error!("Tried to require an unregistered type: {:?}", type_id);
            let _ = response_channel.send(Err(InjectError::RequestedTypeMissing));
            return;
        }

        // Check if the instance is already in the registry
        if let Some(instance) = self.ready_instances.get(&type_id) {
            // If it is, send it immediately - error can be ignored as it just means the receiver was dropped
            let _ = response_channel.send(Ok(instance.clone()));
            return;
        };

        // Otherwise add the request to the waiters list
        let waiters = self.instance_waiters.entry(type_id).or_default();
        waiters.push(response_channel);
    }


    fn handle_app_require(&mut self, response_channel: InjectionResponseSender<App>) {
        self.app_waiters.push(response_channel);
    }

}





struct App {}

//////////////////////////////////////////////////////////////////////
/// 
struct Test {
    a: String,
}
struct TestFactory;
impl InstanceFactory for TestFactory {
    type Provides = Test;

    fn get_dependencies() -> Vec<DependencyInfo> {
        todo!()
    }

    async fn construct(&mut self, di: &InjectionHandle) -> Result<Self::Provides, Infallible> {
        Ok(Test {
            a: "test".to_string(),
        })
    }
}

fn main() {
    let mut registry = AppBuilder::new();
    registry.add_factory(TestFactory);


    println!("{:?}", test_instance);
}




//ToThink
// TODO: Probably should have DI be a different thing as APP - let App contain DiContainer