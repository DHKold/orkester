//! # orkester-plugin-sample
//!
//! Sample plugin demonstrating:
//! - [`ping`]       — simple ping-pong server
//! - [`log_server`] — structured logger with pluggable formatters and consumers
//! - [`rest_server`] — embedded HTTP server with dynamic route registration
//!
//! The plugin uses `export_plugin_root_with_host!` so the root component
//! receives the host pointer; `RestServer` uses it to call back the host router.

mod ping;
mod log_server;
mod rest_server;
mod root;

pub use root::RootComponent;

// Use the host-pointer variant so child factories can receive *mut AbiHost.
orkester_plugin::export_plugin_root_with_host!(RootComponent);

