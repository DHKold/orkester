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
        metrics::NoMetricsServerFactory,
        rest::AxumRestServerFactory,
        state::BasicStateServerFactory,
        workflow::BasicWorkflowServerFactory,
    },
};
use orkester_common::plugin::{ComponentMetadata, Plugin, PluginComponent, PluginMetadata};

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
            ComponentMetadata {
                kind: "auth".to_string(),
                id: "no-auth".to_string(),
                name: "No-op Authentication".to_string(),
                description: "Accepts all requests without authentication.".to_string(),
                builder: PluginComponent::AuthenticationProvider(Box::new(NoAuthProviderBuilder)),
            },
            ComponentMetadata {
                kind: "authz".to_string(),
                id: "basic-authz".to_string(),
                name: "Basic Authorization".to_string(),
                description: "Simple role-based authorization.".to_string(),
                builder: PluginComponent::AuthorizationProvider(Box::new(BasicAuthzProviderBuilder)),
            },
            ComponentMetadata {
                kind: "executor".to_string(),
                id: "dummy-executor".to_string(),
                name: "Dummy Executor".to_string(),
                description: "No-op task executor for testing.".to_string(),
                builder: PluginComponent::ExecutorProvider(Box::new(DummyExecutorBuilder)),
            },
            ComponentMetadata {
                kind: "persistence".to_string(),
                id: "memory-persistence".to_string(),
                name: "In-Memory Persistence".to_string(),
                description: "Volatile in-memory persistence backend.".to_string(),
                builder: PluginComponent::PersistenceProvider(Box::new(MemoryPersistenceBuilder)),
            },
            ComponentMetadata {
                kind: "registry".to_string(),
                id: "local-registry".to_string(),
                name: "Local Workflow Registry".to_string(),
                description: "In-process workflow definition registry.".to_string(),
                builder: PluginComponent::RegistryProvider(Box::new(LocalRegistryBuilder)),
            },
            // ── Servers ─────────────────────────────────────────────────────
            ComponentMetadata {
                kind: "server".to_string(),
                id: "basic-state-server".to_string(),
                name: "Basic State Server".to_string(),
                description: "In-memory workflow state management.".to_string(),
                builder: PluginComponent::Server(Box::new(BasicStateServerFactory)),
            },
            ComponentMetadata {
                kind: "server".to_string(),
                id: "basic-workflow-server".to_string(),
                name: "Basic Workflow Server".to_string(),
                description: "In-process workflow execution engine.".to_string(),
                builder: PluginComponent::Server(Box::new(BasicWorkflowServerFactory)),
            },
            ComponentMetadata {
                kind: "server".to_string(),
                id: "no-metrics-server".to_string(),
                name: "No-op Metrics Server".to_string(),
                description: "Discards all metrics; exposes an empty /metrics endpoint.".to_string(),
                builder: PluginComponent::Server(Box::new(NoMetricsServerFactory)),
            },
            ComponentMetadata {
                kind: "server".to_string(),
                id: "axum-rest-server".to_string(),
                name: "Axum REST Server".to_string(),
                description: "HTTP REST server built on Axum.".to_string(),
                builder: PluginComponent::Server(Box::new(AxumRestServerFactory)),
            },
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
