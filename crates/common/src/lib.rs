/// Plugin registration types and the plugin loading contract.
pub mod plugin;

/// Trait interfaces for all Orkester extensibility points.
pub mod providers;

// Re-export the most commonly used types at crate root.
pub use plugin::{Plugin, PluginComponent, PluginMetadata, PLUGIN_REGISTRATION_SYMBOL};
