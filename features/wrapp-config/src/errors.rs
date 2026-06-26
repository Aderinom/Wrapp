//! Config Errors

use wrapp_di::types::TypeInfo;


/// Errors when trying to register a config
#[derive(thiserror::Error, Debug, Clone)]
pub enum RegisterConfigError {
    /// The required Config is already registered
    #[error("The required Config type is already registered")]
    AlreadyRegistered(TypeInfo),
}
