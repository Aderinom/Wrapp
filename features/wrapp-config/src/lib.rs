//! Wrapp Config consists of the following components:
//!
//! 1. Config - for declairing a struct as a config and handling resolution
//! 2. Provider - for creating a registry of configs, adding and retrieving configs
//! 3. Errors - for config errors

pub mod config;
pub mod errors;
pub mod provider;
