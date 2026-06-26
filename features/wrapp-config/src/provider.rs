//! Config provider to register and retrieve configs based on type.

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::Arc,
};

use wrapp_di::types::TypeInfo;

use crate::errors::{RegisterConfigError};

/// A provider to register all configs.
///
/// Configs can be registered and retrieved based on type.
#[derive(Default)]
pub struct ConfigProvider {
    configs: HashMap<TypeId, Arc<dyn Any + Send + Sync + 'static>>,
}

impl ConfigProvider {
    /// Initializes an empty Config Provider
    #[must_use] 
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
        }
    }

    /// Retrieve a config with specified type.
    pub fn config<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        let type_id = TypeId::of::<T>();

        let config = self.configs
            .get(&type_id)?;

        if let Ok(config) = config.clone().downcast::<T>() { Some(config) } else {
            debug_assert!(false, "Config Provider contained invalid type in slot for type: {:?}", TypeInfo::of::<T>());
            tracing::error!("Config Provider contained invalid type in slot for type: {:?}", TypeInfo::of::<T>());
            None
        }

    }

    /// Add a config to the registry.
    pub fn add_config<T: Send + Sync + 'static>(
        &mut self,
        config: T,
    ) -> Result<&mut Self, RegisterConfigError> {
        let type_id = TypeId::of::<T>();

        if self.configs.contains_key(&type_id) {
            return Err(RegisterConfigError::AlreadyRegistered(TypeInfo::of::<T>()));
        }

        self.configs.insert(type_id, Arc::new(config));
        Ok(self)
    }
}
