use orkester_plugin::{
    abi::AbiHost,
    sdk::{Error, Host, Result},
};
use orkester_macro::component;

use crate::{
    catalog_server::{config::CatalogServerConfig, CatalogServer},    host_client::HostClient,    persistence_server::{
        LocalFsPersistenceConfig, LocalFsPersistenceServer,
        MemoryPersistenceConfig, MemoryPersistenceServer,
    },
    task_runner::{
        runner_components::{
            ContainerRunnerConfig, ContainerRunnerServer,
            KubernetesRunnerConfig, KubernetesRunnerServer,
            ShellRunnerConfig, ShellRunnerServer,
        },
        runner_server::RunnerServer,
    },
    workRunner::{ThreadWorkRunnerConfig, ThreadWorkRunnerServer},
    workflow_server::{config::WorkflowServerConfig, WorkflowServer},
};

// ── RootComponent ─────────────────────────────────────────────────────────────

/// Root component of the workaholic plugin.
///
/// Implemented with the `#[component]` macro.  The host pointer is captured
/// at construction time from `export_plugin_root_with_host!` and forwarded to
/// every child component factory so child components can in turn call back
/// into the host.
pub struct RootComponent {
    host_ptr: *mut AbiHost,
}

// SAFETY: host pointer is valid for the process lifetime and is only read
// inside factory closures (never written).
unsafe impl Send for RootComponent {}

impl RootComponent {
    pub fn new(host_ptr: *mut AbiHost) -> Self {
        Self { host_ptr }
    }

    /// Reconstruct a `Host` from the stored raw pointer.
    ///
    /// # Safety
    /// Must only be called with a pointer that was obtained from `AbiHost`
    /// and that is still valid (i.e. within the plugin's lifetime).
    unsafe fn make_host(&self) -> Host {
        unsafe { Host::from_abi(self.host_ptr) }
    }
}

// ── PluginComponent impl (via macro) ─────────────────────────────────────────
//
// Each `#[factory("kind")]` method is called by the host when it needs to
// create a new instance of that component kind.  The macro generates the
// `to_abi()` body including `.with_factory(...)` calls for every method.

#[component(
    kind        = "workaholic/Root:1.0",
    name        = "WorkaholicRoot",
    description = "Root component of the workaholic workflow plugin."
)]
impl RootComponent {
    // ── Catalog ──────────────────────────────────────────────────────────────

    #[factory("workaholic/CatalogServer:1.0")]
    fn create_catalog_server(&mut self, cfg: CatalogServerConfig) -> Result<CatalogServer> {
        let host = unsafe { self.make_host() };
        Ok(CatalogServer::new(cfg, host))
    }

    // ── Workflow ─────────────────────────────────────────────────────────────

    #[factory("workaholic/WorkflowServer:1.0")]
    fn create_workflow_server(&mut self, cfg: WorkflowServerConfig) -> Result<WorkflowServer> {
        let host = unsafe { self.make_host() };
        Ok(WorkflowServer::new(cfg, host))
    }

    // ── Persistence ───────────────────────────────────────────────────────────

    /// Durable file-system-backed persistence.
    ///
    /// ```yaml
    /// - name: local-fs-persistence
    ///   kind: workaholic/LocalFsPersistence:1.0
    ///   config:
    ///     path: /orkester/bin/data
    /// ```
    #[factory("workaholic/LocalFsPersistence:1.0")]
    fn create_local_fs_persistence(&mut self, cfg: LocalFsPersistenceConfig) -> Result<LocalFsPersistenceServer> {
        let path = cfg.path.clone();
        let server = LocalFsPersistenceServer::new(cfg)
            .map_err(|e| -> Error { e.to_string().into() })?;
        let host = unsafe { self.make_host() };
        HostClient::new(host).log("info", "persistence", &format!("storage root: {path}"));
        Ok(server)
    }

    /// Volatile in-memory persistence (lost on restart; for dev/test).
    ///
    /// ```yaml
    /// - name: memory-persistence
    ///   kind: workaholic/MemoryPersistence:1.0
    /// ```
    #[factory("workaholic/MemoryPersistence:1.0")]
    fn create_memory_persistence(&mut self, cfg: MemoryPersistenceConfig) -> Result<MemoryPersistenceServer> {
        let host = unsafe { self.make_host() };
        HostClient::new(host).log("warn", "persistence", "using volatile in-memory storage");
        Ok(MemoryPersistenceServer::new(cfg))
    }

    // ── Task runners ──────────────────────────────────────────────────────────

    /// Legacy all-in-one runner dispatcher (dispatches by `kind` field).
    ///
    /// ```yaml
    /// - name: runner-server
    ///   kind: workaholic/RunnerServer:1.0
    /// ```
    #[factory("workaholic/RunnerServer:1.0")]
    fn create_runner_server(&mut self, _cfg: serde_json::Value) -> Result<RunnerServer> {
        Ok(RunnerServer)
    }

    /// Shell runner — executes tasks via `sh -c` or a command array.
    ///
    /// ```yaml
    /// - name: shell-runner
    ///   kind: workaholic/ShellRunner:1.0
    /// ```
    #[factory("workaholic/ShellRunner:1.0")]
    fn create_shell_runner(&mut self, _cfg: ShellRunnerConfig) -> Result<ShellRunnerServer> {
        Ok(ShellRunnerServer)
    }

    /// Container runner — executes tasks inside a Docker/Podman container.
    ///
    /// ```yaml
    /// - name: container-runner
    ///   kind: workaholic/ContainerRunner:1.0
    /// ```
    #[factory("workaholic/ContainerRunner:1.0")]
    fn create_container_runner(&mut self, _cfg: ContainerRunnerConfig) -> Result<ContainerRunnerServer> {
        Ok(ContainerRunnerServer)
    }

    /// Kubernetes runner — executes tasks as Kubernetes Jobs.
    ///
    /// ```yaml
    /// - name: k8s-runner
    ///   kind: workaholic/KubernetesRunner:1.0
    /// ```
    #[factory("workaholic/KubernetesRunner:1.0")]
    fn create_kubernetes_runner(&mut self, _cfg: KubernetesRunnerConfig) -> Result<KubernetesRunnerServer> {
        Ok(KubernetesRunnerServer)
    }

    // ── WorkRunners ───────────────────────────────────────────────────────────────

    /// Standalone thread-pool workRunner component.
    ///
    /// ```yaml
    /// - name: main-workRunner
    ///   kind: workaholic/ThreadWorkRunner:1.0
    ///   config:
    ///     max_work_runs: 4
    ///     persistence: local-fs-persistence
    ///     runner_mappings:
    ///       - kind: shell
    ///         component: shell-runner
    /// ```
    #[factory("workaholic/ThreadWorkRunner:1.0")]
    fn create_thread_workRunner(&mut self, cfg: ThreadWorkRunnerConfig) -> Result<ThreadWorkRunnerServer> {
        let host = unsafe { self.make_host() };
        Ok(ThreadWorkRunnerServer::new(cfg, host))
    }
}

