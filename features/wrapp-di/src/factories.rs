use std::{convert::Infallible, future::Future, pin::Pin};

use crate::{
    initiator::DiHandle,
    types::{DependencyInfo, DynError, Injectable, Instance, TypeInfo},
};

/// A Factory providing instances of a given type
pub trait InstanceFactory: Send + Sync {
    type Provides: Injectable;

    /// Returns the typeinfo about the factory's provided type
    fn supplies() -> TypeInfo {
        TypeInfo::of::<Self::Provides>()
    }

    /// Returns a list of dependencies the factory requires to supply it's type
    fn get_dependencies() -> Vec<DependencyInfo>;

    /// Constructs a new instance of the factory's provided type
    ///
    /// Returns the constructed instance, or an error if either Dependencies are not satisfied or the Instantiation failed
    fn construct(
        &mut self,
        di: DiHandle,
    ) -> impl Future<Output = Result<Self::Provides, impl Into<DynError>>> + Send + '_;

    /// Returns a boolean indicating whether the factory is enabled or not
    fn is_enabled(
        &mut self,
        di: DiHandle,
    ) -> impl Future<Output = Result<bool, impl Into<DynError>>> + Send + '_ {
        di; // Ignore unused
        async { Ok::<_, Infallible>(true) }
    }
}

/// Wrapper Trait for factories, providing instances of Any
pub trait DynFactory {
    fn supplies(&self) -> TypeInfo;

    /// Returns a list of dependencies for the factory
    fn dependencies(&self) -> Vec<DependencyInfo>;

    /// Constructs a new instance of the factory's provided type, fulfilling all its dependencies
    fn construct(
        &mut self,
        di: DiHandle,
    ) -> Box<dyn Future<Output = Result<Instance, DynError>> + Send + '_>;

    /// Returns a boolean indicating whether the factory is enabled or not
    fn is_enabled(
        &mut self,
        di: DiHandle,
    ) -> Box<dyn Future<Output = Result<bool, DynError>> + Send + '_>;
}
// Impl DynFactory for any InstanceFactory
impl<T: Injectable, SpecificFactory: InstanceFactory<Provides = T>> DynFactory for SpecificFactory {
    fn supplies(&self) -> TypeInfo {
        SpecificFactory::supplies()
    }

    fn dependencies(&self) -> Vec<DependencyInfo> {
        SpecificFactory::get_dependencies()
    }

    fn construct(
        &mut self,
        di: DiHandle,
    ) -> Box<dyn Future<Output = Result<Instance, DynError>> + Send + '_> {
        let construction_fut = async {
            // Forward the call to the specific implementation
            SpecificFactory::construct(self, di)
                .await
                .map(Instance::new)
                .map_err(|e| e.into())
        };

        Box::new(construction_fut)
    }

    fn is_enabled(
        &mut self,
        di: DiHandle,
    ) -> Box<dyn Future<Output = Result<bool, DynError>> + Send + '_> {
        let future = async {
            // Forward the call to the specific implementation
            SpecificFactory::is_enabled(self, di)
                .await
                .map_err(|e| e.into())
        };

        Box::new(future)
    }
}
