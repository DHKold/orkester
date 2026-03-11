//! Logging setup and utilities

pub mod config;
pub mod init;

pub use config::{LoggingConfig, LogFormat};
pub use init::init;
