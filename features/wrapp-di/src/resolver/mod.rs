use crate::{errors::InjectError, initiator::DiHandle, types::DependencyInfo};

pub mod arc;
pub mod lazy;

/// Allows custom behaviour on injection
pub trait Resolver {
    #[allow(async_fn_in_trait)]
    async fn resolve(handle: &mut DiHandle) -> Result<Self, InjectError>
    where
        Self: Sized;

    fn dependency_info() -> DependencyInfo;
}
