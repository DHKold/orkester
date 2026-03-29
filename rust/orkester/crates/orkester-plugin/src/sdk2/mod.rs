// ==== Plugin SDK v2 ====

use crate::std::{Document, DocumentMetadata};

/// The plugin document type, with the standard `metadata`, `spec`, and `status` fields.
pub type PluginDoc = Document<DocumentMetadata, PluginSpec, PluginStatus>;

/// The `spec` field of a plugin document, containing static plugin information.
pub struct PluginSpec {
    /// List of components provided by this plugin, each with its own set of actions.
    pub components: Vec<ComponentBrief>,
}

/// The `status` field of a plugin document, containing dynamic plugin information that can be updated at runtime.
pub struct PluginStatus {
    // Empty for now, but can be extended in the future for runtime status reporting.
}

/// Brief information about a component, used for discovery and registration in the plugin runtime.
pub struct ComponentBrief {
    /// Unique name of the component within this plugin.
    name: String,
    /// Version of the component, following semantic versioning (e.g., "1.0.0").
    version: String,
    /// Description of the component's functionality.
    description: String,
    /// List of actions supported by this component, each with its own name, version, and description.
    actions: Vec<ActionBrief>,
}

/// Brief information about an action, used for discovery and invocation in the plugin runtime.
pub struct ActionBrief {
    /// Unique name of the action within its component.
    name: String,
    /// Version of the action, following semantic versioning (e.g., "1.0.0").
    version: String,
    /// Description of the action's functionality.
    description: String,
}

pub trait Documentable<M, S, T> {
    fn to_document(&self) -> Document<M, S, T>
    where
        M: serde::Serialize,
        S: serde::Serialize,
        T: serde::Serialize;
}

/// Configuration for constructing a plugin, supplied by the host environment at load time.
pub struct PluginConfig {
    /// Would probably contain required config related to the host environment, such as supported API versions, capabilities, etc.
    /// For simplicity, we'll leave it empty for now, but it can be extended in the future as needed.
}

/// The main plugin trait that all plugins must implement.
pub trait Plugin: Sized + Send + Sync + Documentable<DocumentMetadata, PluginSpec, PluginStatus> {
    fn new(config: PluginConfig) -> Self where Self: Sized;
}