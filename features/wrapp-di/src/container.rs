use std::{
    any::{type_name, TypeId},
    collections::HashMap,
    fmt::Debug,
    sync::Arc,
};

use crate::{
    dependency_graph::DependencyGraph,
    errors::RequireError,
    types::{Injectable, Instance, TypeInfo},
};

/// Container holding all initiated instances
#[derive(Clone)]
pub struct DiContainer(pub Arc<DiContainerInner>);
pub struct DiContainerInner {
    instances: HashMap<TypeId, (TypeInfo, Option<Instance>)>,
    graph: DependencyGraph,
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
            map.field(info.type_name, &val);
        }
        map.finish()
    }
}

impl DiContainer {
    pub(crate) fn new(
        instances: HashMap<TypeId, (TypeInfo, Option<Instance>)>,
        graph: DependencyGraph,
    ) -> Self {
        Self(Arc::new(DiContainerInner { instances, graph }))
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

    pub fn graph(&self) -> &DependencyGraph {
        &self.0.graph
    }
}
