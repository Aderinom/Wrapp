use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::Arc,
};

use crate::errors::ConfigError;

/// A provider to register all configs.
///
/// Configs can be registered and retrieved based on type.
pub struct ConfigProvider {
    configs: HashMap<TypeId, Arc<dyn Any + Send + Sync + 'static>>,
}

impl ConfigProvider {
    /// Initializes an empty Config Provider
    pub fn initialize() -> Self {
        Self {
            configs: HashMap::new(),
        }
    }

    /// Retrieve a config with specified type.
    ///
    /// If the config type is not available, it will return a [`ConfigError`] runtime error
    pub fn get_config<T: Send + Sync + 'static>(&self) -> Result<Option<Arc<T>>, ConfigError> {
        let type_id = TypeId::of::<T>();

        self.configs
            .get(&type_id)
            .map(|entry| entry.clone().downcast())
            .transpose()
            .map_err(|_| ConfigError::ConfigMissing(type_id))
    }

    /// Add a config to the registry.
    ///
    /// If the config type is already registered, it will return a
    /// [`ConfigError`] runtime error
    pub fn add_config<T: Send + Sync + 'static>(
        &mut self,
        config: T,
    ) -> Result<&mut Self, ConfigError> {
        let type_id = TypeId::of::<T>();

        if self.configs.contains_key(&type_id) {
            return Err(ConfigError::ConfigAlreadyRegistered(type_id));
        }

        self.configs.insert(type_id, Arc::new(config));
        Ok(self)
    }

    /// Can optionally add a config to the registry.
    ///
    /// If the config provided is `Some(T)`, it will be the same as calling [`ConfigProvider::add_config`]
    /// If the config provided is `None`, then the function just returns `Ok(self)` for chaining
    pub fn maybe_add_config<T: Send + Sync + 'static>(
        &mut self,
        config: Option<T>,
    ) -> Result<&mut Self, ConfigError> {
        match config {
            Some(c) => self.add_config(c),
            None => Ok(self),
        }
    }
}
