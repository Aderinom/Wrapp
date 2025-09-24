use std::{
    any::{Any, TypeId},
    sync::Arc,
};

/// All errors must be clone
pub type DynError = Box<dyn std::error::Error + Send + Sync>;

/// We assume that we are using a multithreaded async runtime
/// So anything injectable needs to be Send + Sync + 'static
pub trait Injectable: Send + Sync + 'static {}
impl<T: Send + Sync + 'static> Injectable for T {}

/// Instance of a Provider
#[derive(Clone)]
pub struct Instance {
    pub info: TypeInfo,
    pub instance: Arc<dyn Any + Send + Sync + 'static>,
}

impl Instance {
    pub(crate) fn new<ExistingInstance: Injectable>(instance: ExistingInstance) -> Self {
        Instance {
            info: TypeInfo::of::<ExistingInstance>(),
            instance: Arc::new(instance),
        }
    }

    pub fn downcast<T: Injectable>(&self) -> Result<Arc<T>, &'static str> {
        match Arc::downcast::<T>(self.instance.clone()) {
            Ok(downcasted) => Ok(downcasted),
            Err(_) => Err(self.info.type_name),
        }
    }
}

/// Information about a Factory dependency
pub struct DependencyInfo {
    /// The required Type
    pub type_info: TypeInfo,
    /// If it is optional or required
    pub optional: bool,
    /// If the Dependency is injected lazily
    pub lazy: bool,
}

/// Type Name and Type Id
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct TypeInfo {
    pub type_name: &'static str,
    pub type_id: TypeId,
}
impl std::fmt::Display for TypeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.type_name)
    }
}
impl TypeInfo {
    pub fn of<T: 'static + ?Sized>() -> TypeInfo {
        TypeInfo {
            type_name: std::any::type_name::<T>(),
            type_id: TypeId::of::<T>(),
        }
    }
}
