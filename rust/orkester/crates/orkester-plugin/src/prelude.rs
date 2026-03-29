/// Convenience re-exports for plugin authors.
///
/// `use orkester_plugin::prelude::*;` brings into scope everything needed to
/// write a plugin without any further explicit imports.
pub use crate::abi::AbiRequest;
pub use crate::sdk::{
    Error, Result,
    AbiComponentBuilder, ComponentMetadata, Host, HostRef, PluginComponent,
};
pub use crate::{component, export_plugin_root};
