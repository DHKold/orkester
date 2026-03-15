pub mod auth;
pub mod authz;
pub mod executor;
pub mod persistence;
pub mod registry;
pub mod servers;

use crate::{
    auth::NoAuthProviderBuilder,
    authz::BasicAuthzProviderBuilder,
    executor::{CommandsExecutorBuilder, DummyExecutorBuilder},
    persistence::{FilePersistenceBuilder, MemoryPersistenceBuilder},
    registry::LocalRegistryBuilder,
    servers::{
        metrics::MetricsServerBuilder, rest::AxumRestServerBuilder,
        workflows::WorkflowsServerBuilder, workspace::WorkspaceServerBuilder,
    },
};
use orkester_common::logging::Logger;
use orkester_common::plugin::{ComponentMetadata, Plugin, PluginComponent, PluginMetadata};

/// Constructs the core plugin, bundling all built-in provider and server implementations.
pub fn core_plugin() -> Plugin {
    Plugin {
        metadata: PluginMetadata {
            id: "orkester-plugin-core".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "Built-in default implementations: NoAuth, BasicAuthz, Dummy executor, \
                          in-memory persistence, local workflow registry, BasicStateServer, \
                          BasicWorkflowServer, MetricsServer, and AxumRestServer."
                .to_string(),
            authors: vec!["Orkester Contributors".to_string()],
        },
        components: vec![
            // ── Providers ───────────────────────────────────────────────────
            ComponentMetadata {
                kind: "auth".to_string(),
                id: "no-auth".to_string(),
                description: "Accepts all requests without authentication.".to_string(),
                builder: PluginComponent::AuthenticationProvider(Box::new(NoAuthProviderBuilder)),
            },
            ComponentMetadata {
                kind: "authz".to_string(),
                id: "basic-authz".to_string(),
                description: "Simple role-based authorization.".to_string(),
                builder: PluginComponent::AuthorizationProvider(Box::new(
                    BasicAuthzProviderBuilder,
                )),
            },
            ComponentMetadata {
                kind: "executor".to_string(),
                id: "dummy".to_string(),
                description: "No-op task executor for testing.".to_string(),
                builder: PluginComponent::ExecutorProvider(Box::new(DummyExecutorBuilder)),
            },
            ComponentMetadata {
                kind: "executor".to_string(),
                id: "commands".to_string(),
                description: "Runs shell commands locally; captures stdout/stderr as outputs."
                    .to_string(),
                builder: PluginComponent::ExecutorProvider(Box::new(CommandsExecutorBuilder)),
            },
            ComponentMetadata {
                kind: "persistence".to_string(),
                id: "memory-persistence".to_string(),
                description: "Volatile in-memory persistence backend.".to_string(),
                builder: PluginComponent::PersistenceProvider(Box::new(MemoryPersistenceBuilder)),
            },
            ComponentMetadata {
                kind: "persistence".to_string(),
                id: "file-persistence".to_string(),
                description: "Durable file-based persistence backend; stores entities as JSON files on disk.".to_string(),
                builder: PluginComponent::PersistenceProvider(Box::new(FilePersistenceBuilder)),
            },
            ComponentMetadata {
                kind: "registry".to_string(),
                id: "local-registry".to_string(),
                description: "In-process workflow definition registry.".to_string(),
                builder: PluginComponent::RegistryProvider(Box::new(LocalRegistryBuilder)),
            },
            // ── Servers ─────────────────────────────────────────────────────
            ComponentMetadata {
                kind: "server".to_string(),
                id: "metrics-server".to_string(),
                description: "Exposes runtime counters (uptime, request counts) at GET /v1/metrics."
                    .to_string(),
                builder: PluginComponent::Server(Box::new(MetricsServerBuilder)),
            },
            ComponentMetadata {
                kind: "server".to_string(),
                id: "axum-rest-server".to_string(),
                description: "HTTP REST server built on Axum.".to_string(),
                builder: PluginComponent::Server(Box::new(AxumRestServerBuilder)),
            },
            ComponentMetadata {
                kind: "server".to_string(),
                id: "workspace-server".to_string(),
                description: "Loads and exposes Namespace, Task, and Work objects.".to_string(),
                builder: PluginComponent::Server(Box::new(WorkspaceServerBuilder)),
            },
            ComponentMetadata {
                kind: "server".to_string(),
                id: "workflows-server".to_string(),
                description: "Manages Crons, schedules, and Workflow execution.".to_string(),
                builder: PluginComponent::Server(Box::new(WorkflowsServerBuilder)),
            },
        ],
    }
}

/// Well-known dynamic-loading entry point.
/// When loaded as a shared library, Orkester will call this symbol to obtain the plugin.
///
/// # Safety
/// The returned pointer is heap-allocated and ownership is transferred to the caller.
#[no_mangle]
pub extern "C" fn orkester_register_plugin() -> *mut Plugin {
    Box::into_raw(Box::new(core_plugin()))
}

/// Logger-injection entry point called by Orkester right after loading this library.
///
/// Redirects all `log_*!` calls in this plugin to the host process's global logger
/// so they reach the consumers configured in `core`.
///
/// # Safety
/// `logger` must be a valid pointer to a [`Logger`] that lives for the entire
/// process lifetime (i.e. the host's `Logger::global()`).
#[no_mangle]
pub unsafe extern "C" fn orkester_set_logger(logger: *const Logger) {
    Logger::inject(logger);
}
