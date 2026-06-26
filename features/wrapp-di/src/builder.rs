use std::{collections::HashMap, time::Duration};

use crate::{
    container::DiContainer,
    errors::InitError,
    factories::{DynFactory, InstanceFactory},
    initiator::DiInitiator,
    types::{Injectable, Instance, TypeInfo},
};

//////////////////////////////////////////////////////////////////////
///
/// The DI Consist of three Parts.
/// 1. The AppBuilder where one registers all factories and instances
/// 2. Then for initialization



pub struct DiBuilder {
    /// Registered factories which can provide instances
    pub(crate) registered_factories: Vec<Box<dyn DynFactory>>,
    /// Registered already created instances
    pub(crate) registered_instances: HashMap<TypeInfo, Instance>,
}
impl Default for DiBuilder {
    fn default() -> Self {
        Self::new()
    }
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
