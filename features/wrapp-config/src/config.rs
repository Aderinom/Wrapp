use std::{any::type_name, ops::Deref, sync::Arc};

use wrapp_di::{
    errors::{InjectError, RequireError},
    initiator::DiHandle,
    resolver::Resolver,
    types::{DependencyInfo, TypeInfo},
};

use crate::provider::ConfigProvider;

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
