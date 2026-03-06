/// Core domain types: Workspace, Work, Task, Artifact, Execution, etc.
pub mod domain;

/// Plugin registration types and the plugin loading contract.
pub mod plugin;

/// Trait interfaces for all Orkester provider extensibility points.
pub mod providers;

/// Trait interfaces for all Orkester server components.
pub mod servers;

// Re-export the most commonly used types at crate root.
pub use plugin::{Plugin, PluginComponent, PluginMetadata, PLUGIN_REGISTRATION_SYMBOL};
