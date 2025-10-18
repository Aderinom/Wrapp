//! Wrapp Config provides a global registry of configs that can be injected in the rest of the
//! modules.
//!
//! Wrapp Config is split into two major parts:
//! 1. ConfigProvider: Used to create the registry of all configs
//! 2. Config<T>: A wrapper type to be able to resolve and retrieve configs
//!
//! # Examples
//!
//! ```rust
//! #[derive(Clone)]
//! struct AppConfig {
//!     host: String,
//!     port: u16,
//!     app_name: String,
//! }
//!
//! fn setup_config() {
//!     let app_config = AppConfig {
//!         host: "localhost".to_string(),
//!         port: 8080_u16,
//!         app_name: "My Awesome App".to_string(),
//!     };
//!
//!     let mut config_provider = ConfigProvider::default();
//!     let config_provider = match config_provider.add_config(app_config.clone()) {
//!         Ok(p) => p,
//!         Err(e) => {
//!             eprintln!("{e}");
//!             return;
//!         }
//!     };
//!
//!     let retrieved_config = match config_provider.get_config::<AppConfig>() {
//!         Ok(Some(c)) => c,
//!         Ok(None) => {
//!             eprintln!("Could not find config type");
//!             return;
//!         }
//!         Err(e) => {
//!             eprintln!("{e}");
//!             return;
//!         }
//!     };
//!
//!     assert_eq!(app_config.host, retrieved_config.host);
//!     assert_eq!(app_config.port, retrieved_config.port);
//!     assert_eq!(app_config.app_name, retrieved_config.app_name);
//! }
//!
//! ```
//!
//! Wrapp Config consists of the following components:
//!
//! 1. Config - for declairing a struct as a config and handling resolution
//! 2. Provider - for creating a registry of configs, adding and retrieving configs
//! 3. Errors - for config errors

pub mod config;
pub mod errors;
pub mod provider;
