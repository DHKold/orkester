//! # orkester-plugin-sample
//!
//! An example plugin that demonstrates how to author Orkester components.
//!
//! ## Components
//! - [`logger`] — configurable structured logger with Console and File backends
//! - [`calculator`] — four-operation calculator
//! - [`counter`] — in-memory counter (increment/decrement/reset/get)
//! - [`echo`] — simple message echo with an optional prefix
//!
//! ## Plugin root
//! [`RootComponent`] is the entry point; it exposes all other components as
//! factories.  The plugin is exported via `export_plugin_root!(RootComponent)`.

pub mod calculator;
pub mod counter;
pub mod echo;
pub mod logger;
mod root;

pub use root::RootComponent;

orkester_plugin::export_plugin_root!(RootComponent);
