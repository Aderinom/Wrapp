use std::{collections::HashMap, time::Duration};

use crate::{
    container::DiContainer,
    errors::InitError,
    factories::{DynFactory, InstanceFactory},
    initiator::DiInitiator,
    types::{Injectable, Instance, TypeInfo},
};

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
    #[must_use]
    pub fn new() -> Self {
        Self {
            registered_factories: Vec::new(),
            registered_instances: HashMap::new(),
        }
    }
}
impl DiBuilder {
    /// Adds an instance of a type to the DI container.
    ///
    /// An instance is a concrete object that has already been created and is ready to be used.
    /// These instances are stored in the DI container and can be retrieved by their type during
    /// container initialization.
    #[must_use]
    pub fn add_instance<T: Injectable>(mut self, instance: T) -> Self {
        self.registered_instances
            .insert(TypeInfo::of::<T>(), Instance::new(instance));
        self
    }

    /// Adds a factory to the DI container.
    ///
    /// [Factories](InstanceFactory) are responsible for creating instances of a specific type
    /// during initialization of the DI container. They can have dependencies on other types,
    /// which will be resolved and injected by the DI container during [building](DiBuilder::build).
    #[must_use]
    pub fn add_factory<Factory: InstanceFactory + 'static>(mut self, factory: Factory) -> Self {
        self.registered_factories.push(Box::new(factory));
        self
    }

    /// Builds the DI container, resolving all dependencies and creating instances as needed.
    ///
    /// ### Errors
    /// - Fails if any factory fails to create an instance.
    /// - Fails if there are conflicts in the dependency graph, e.g. circular dependencies or
    ///   missing required types.
    /// - Fails if two factories produce the same type, which would create ambiguity in the
    ///   dependency graph.
    pub async fn build(self) -> Result<DiContainer, InitError> {
        DiInitiator::new().initiate(self, None).await
    }

    /// Builds the DI container, resolving all dependencies and creating instances as needed.
    ///
    /// ### Errors
    /// - Fails if any factory fails to create an instance.
    /// - Fails if there are conflicts in the dependency graph, e.g. circular dependencies or
    ///   missing required types.
    /// - Fails if two factories produce the same type, which would create ambiguity in the
    ///   dependency graph.
    /// - Fails if the initialization takes longer than the specified timeout duration.
    pub async fn build_timeout(self, timeout: Duration) -> Result<DiContainer, InitError> {
        DiInitiator::new().initiate(self, Some(timeout)).await
    }
}
