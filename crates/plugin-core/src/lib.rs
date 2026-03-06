pub mod auth;
pub mod authz;
pub mod executor;
pub mod persistence;
pub mod registry;

use orkester_common::plugin::{Plugin, PluginComponent, PluginMetadata};
use crate::{
    auth::NoAuthProviderBuilder,
    authz::BasicAuthzProviderBuilder,
    executor::DummyExecutorBuilder,
    persistence::MemoryPersistenceBuilder,
    registry::LocalRegistryBuilder,
};

/// Constructs the core plugin, bundling all built-in provider builders.
pub fn core_plugin() -> Plugin {
    Plugin {
        metadata: PluginMetadata {
            id: "orkester-plugin-core".to_string(),
            name: "Orkester Core Plugin".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "Built-in default implementations: NoAuth, BasicAuthz, Dummy executor, \
                          in-memory persistence, and local workflow registry."
                .to_string(),
            authors: vec!["Orkester Contributors".to_string()],
        },
        components: vec![
            PluginComponent::Authentication(Box::new(NoAuthProviderBuilder)),
            PluginComponent::Authorization(Box::new(BasicAuthzProviderBuilder)),
            PluginComponent::TaskExecutor(Box::new(DummyExecutorBuilder)),
            PluginComponent::Persistence(Box::new(MemoryPersistenceBuilder)),
            PluginComponent::WorkflowRegistry(Box::new(LocalRegistryBuilder)),
        ],
    }
}

/// Well-known dynamic-loading entry point.
/// When loaded as a shared library, Orkester will call this symbol to obtain the plugin.
///
/// # Safety
/// The returned pointer is heap-allocated and ownership is transferred to the caller.
#[unsafe(no_mangle)]
pub extern "C" fn orkester_register_plugin() -> *mut Plugin {
    Box::into_raw(Box::new(core_plugin()))
}
