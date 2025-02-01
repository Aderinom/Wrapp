use std::{
    any::{Any, TypeId},
    collections::HashMap,
    convert::Infallible,
    future::Future,
    sync::Arc,
};

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
        di: InjectionHandle,
    ) -> Result<Self::Provides, impl Into<DynError>>;
    /// Returns a boolean indicating whether the factory is enabled or not
    async fn is_enabled(&mut self, di: InjectionHandle) -> Result<bool, impl Into<DynError>> {
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
        di: InjectionHandle,
    ) -> Box<dyn Future<Output = Result<Instance, DynError>>>;
    /// Returns a boolean indicating whether the factory is enabled or not
    fn is_enabled(
        &mut self,
        di: InjectionHandle,
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
        di: InjectionHandle,
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
        di: InjectionHandle,
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

struct ApplicationBuilder {
    /// Registered factories which can provide instances
    registered_factories: Vec<Box<dyn DynFactory>>,
    /// Registered already created instances
    registered_instances: HashMap<TypeId, Instance>,
}
impl ApplicationBuilder {
    pub fn new() -> Self {
        ApplicationBuilder {
            registered_factories: Vec::new(),
            registered_instances: HashMap::new(),
        }
    }
}

impl ApplicationBuilder {
    pub fn add_instance<T: 'static>(&mut self, instance: T) {
        self.registered_instances
            .insert(TypeId::of::<T>(), Instance::new(instance));
    }

    pub fn add_factory<Factory: InstanceFactory + 'static>(&mut self, factory: Factory) {
        self.registered_factories.push(Box::new(factory));
    }
}

trait InjectionHandleStrategy {
    type Outputs;
    fn resolve(handle: &InjectionHandle) -> Self::Outputs;
}

/// DI Handle for resolving dependencies and getting instances from the registry.
/// The DI Handle is only valid during instanciation of the Application.
/// Afterwards the DI Container can be used directly for dependency injection.
struct InjectionHandle;

// impl DiHandle {
//     async fn resolve<T>(&self, type_name: &str) -> Result<Instance, DiHandleError> {

//     }

//     async fn resolve_lazy<T>(&self, type_name: &str) -> Result<Instance, DiHandleError> {

//     }

//     async fn resolve_using_strategy<Strategy: DiHandleStrategy>(&self) -> Result<Strategy::Outputs, DiHandleError> {
//         Strategy::resolve(self)
//     }
// }

enum InjectonError {
    Todo,
}

enum InjectionRequests {
    Require {
        type_id: TypeId,
        response_channel: futures_channel::oneshot::Sender<Result<Instance, InjectonError>>,
    },
    RequireLazy {
        type_id: TypeId,
        response_channel: futures_channel::oneshot::Sender<Result<Container, InjectonError>>,
    },
}

struct AppInitiator {
    injection_request_receiver: mpsc::Receiver<InjectionRequests>,
}

struct Container {}

//////////////////////////////////////////////////////////////////////
///
///
///
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

    async fn construct(&mut self, di: InjectionHandle) -> Result<Self::Provides, Infallible> {
        Ok(Test {
            a: "test".to_string(),
        })
    }
}

fn main() {
    let mut registry = ApplicationBuilder::new();
    registry.add_factory(TestFactory);

    println!("{:?}", test_instance);
}
