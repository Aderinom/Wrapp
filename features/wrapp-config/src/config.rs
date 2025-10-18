use std::{any::type_name, ops::Deref, sync::Arc};

use wrapp_di::{
    errors::{InjectError, RequireError},
    initiator::DiHandle,
    resolver::Resolver,
    types::{DependencyInfo, TypeInfo},
};

use crate::provider::ConfigProvider;

/// A wrapper type to allow for config injections
///
/// This provides a simple way to retrieve configs from the config registry,
/// and inject them on a factory as a dependency
///
/// # Example
/// ```rust
/// #[derive(Clone)]
/// pub struct MyModuleConfig {
///     enabled: bool,
///     ...
/// }
///
/// fn register_config() {
///     let config_provider = ConfigProvider::new();
///     let my_module_config = MyModuleConfig {
///         enabled: true,
///         ...
///     };
///
///     config_provider.add_config(my_module_config).unwrap();
/// }
///
///
/// #[wrapp::module]
/// pub struct MyModule;
/// impl MyModule {
///     #[wrapp::module(condition)]
///     pub fn enable(config: Config<MyModuleConfig>) -> bool {
///         config.enabled
///     }
/// }
///
/// ```
pub struct Config<T> {
    inner: Arc<T>,
}
impl<T> Deref for Config<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<T> Config<T> {
    pub fn inner(&self) -> Arc<T> {
        self.inner.clone()
    }

    pub fn into_inner(self) -> Arc<T> {
        self.inner
    }
}

impl<T: Send + Sync + 'static> Resolver for Config<T> {
    async fn resolve(handle: &mut DiHandle) -> Result<Self, InjectError>
    where
        Self: Sized,
    {
        let config_name = type_name::<T>();
        let config_provider = handle.resolve::<Arc<ConfigProvider>>().await?;

        let config: Arc<T> = config_provider
            .get_config()
            .map_err(|e| InjectError::Other(Box::new(e)))?
            .ok_or_else(|| InjectError::RequireError(RequireError::TypeMissing(config_name)))?;

        Ok(Config { inner: config })
    }

    fn dependency_info() -> DependencyInfo {
        DependencyInfo {
            type_info: TypeInfo::of::<Config<T>>(),
            optional: false,
            lazy: false,
        }
    }
}
