//! Wrapp Config provides a simple config injection mechanism for Wrapp DI.
//!
//! ### Overview
//!
//! - [`ConfigProvider`](crate::provider::ConfigProvider) - Registry of configs which can be
//!   injected into modules.
//! - [`Config<ConfigType>`](crate::resolver::Config) - [`Resolver`](wrapp_di::resolver::Resolver)
//!   type which allows for config injections in factories.
//!
//!
//! # Examples
//! ```rust
#![doc = include_str!("../examples/using-config-provider.rs")]
//! ```

pub mod errors;
pub mod provider;
pub mod resolver;
