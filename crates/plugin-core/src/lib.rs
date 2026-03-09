pub mod auth;
pub mod authz;
pub mod executor;
pub mod persistence;
pub mod registry;
pub mod servers;

use crate::{
    auth::NoAuthProviderBuilder,
    authz::BasicAuthzProviderBuilder,
    executor::DummyExecutorBuilder,
    persistence::MemoryPersistenceBuilder,
    registry::LocalRegistryBuilder,
    servers::{
        metrics::{metrics_api_contributor, NoMetricsServerFactory},
        rest::AxumRestServerFactory,
        state::BasicStateServerFactory,
        workflow::BasicWorkflowServerFactory,
    },
};
use orkester_common::plugin::{Plugin, PluginComponent, PluginMetadata};

/// Constructs the core plugin, bundling all built-in provider and server implementations.
pub fn core_plugin() -> Plugin {
    Plugin {
        metadata: PluginMetadata {
            id: "orkester-plugin-core".to_string(),
            name: "Orkester Core Plugin".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "Built-in default implementations: NoAuth, BasicAuthz, Dummy executor, \
                          in-memory persistence, local workflow registry, BasicStateServer, \
                          BasicWorkflowServer, NoMetricsServer, and AxumRestServer."
                .to_string(),
            authors: vec!["Orkester Contributors".to_string()],
        },
        components: vec![
            // ── Providers ───────────────────────────────────────────────────
            PluginComponent::Authentication(Box::new(NoAuthProviderBuilder)),
            PluginComponent::Authorization(Box::new(BasicAuthzProviderBuilder)),
            PluginComponent::TaskExecutor(Box::new(DummyExecutorBuilder)),
            PluginComponent::Persistence(Box::new(MemoryPersistenceBuilder)),
            PluginComponent::WorkflowRegistry(Box::new(LocalRegistryBuilder)),
            // ── Servers ─────────────────────────────────────────────────────
            PluginComponent::StateServer(Box::new(BasicStateServerFactory)),
            PluginComponent::WorkflowServer(Box::new(BasicWorkflowServerFactory)),
            PluginComponent::MetricsServer(Box::new(NoMetricsServerFactory)),
            PluginComponent::RestServer(Box::new(AxumRestServerFactory)),
            // ── API contributors ─────────────────────────────────────────────
            PluginComponent::ApiContributor(Box::new(metrics_api_contributor())),
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
