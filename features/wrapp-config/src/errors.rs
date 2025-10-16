use std::any::TypeId;

/// Errors when trying to require a certain config
#[derive(thiserror::Error, Debug, Clone)]
pub enum ConfigError {
    /// The required Config is not known
    #[error("The required Config type is not known")]
    ConfigMissing(TypeId),

    /// The required Config is already registered
    #[error("The required Config type is already registered")]
    ConfigAlreadyRegistered(TypeId),
}
