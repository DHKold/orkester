//! `workaholic/ThreadWorkRunner:1.0` — a standalone thread-workRunner component.
//!
//! This component wraps a [`ThreadWorkRunner`] background thread in the plugin
//! ABI so it can be registered as a first-class server in the host config:
//!
//! ```yaml
//! servers:
//!   - name: main-workRunner
//!     kind: workaholic/ThreadWorkRunner:1.0
//!     config:
//!       max_work_runs: 4
//!       persistence: local-fs-persistence   # component name
//!       runner_mappings:
//!         - kind: shell
//!           component: shell-runner
//! ```
//!
//! The component exposes two actions.  Because the action namespace is derived
//! from the component's registered **name** by the routing host, you can
//! register multiple workRunners with different names:
//!
//! | registered name | action namespace | action             |
//! |-----------------|------------------|--------------------|
//! | `main-workRunner`   | `main-workRunner`    | `main-workRunner/Enqueue`  |
//! | `analytics-workRunner` | `analytics-workRunner` | `analytics-workRunner/Enqueue` |
//!
//! To route to a specific workRunner, callers use
//! `host.call("main-workRunner/Enqueue", ...)`.
//!
//! **Limitation (v1):** the `#[handle]` macro requires compile-time action
//! strings, so this component is implemented manually using
//! `AbiComponentBuilder::with_handler`.  The action names are built from the
//! configured workRunner name at construction time.

use std::sync::Mutex;

use orkester_plugin::{
    sdk::{AbiComponentBuilder, ComponentMetadata, Host, PluginComponent},
};
use serde::{Deserialize, Serialize};

use crate::{
    host_client::HostClient,
    persistence_server::PersistenceClient,
};
use super::{WorkRunnerConfig, WorkRunnerContext, WorkRunnerHandle, thread::ThreadWorkRunner};

// ── Wire types ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct EnqueueRequest {
    pub work_run_id: String,
}

#[derive(Serialize)]
pub struct EnqueueAck {
    pub ok: bool,
    pub queued_to: String,
}

#[derive(Deserialize)]
pub struct WorkRunnerStatusRequest {}

#[derive(Serialize)]
pub struct WorkRunnerStatusResponse {
    pub name: String,
    pub active: usize,
    pub capacity: usize,
    pub has_capacity: bool,
}

// ── Config ────────────────────────────────────────────────────────────────────

/// Configuration for `workaholic/ThreadWorkRunner:1.0`.
#[derive(Debug, Deserialize)]
pub struct ThreadWorkRunnerConfig {
    /// Logical name used in log messages (defaults to the component's
    /// registered host name if not set).
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_max_work_runs")]
    pub max_work_runs: usize,
    #[serde(default = "default_max_task_runs")]
    pub max_task_runs: usize,
    /// Registered name of the persistence component.
    #[serde(default = "default_persistence")]
    pub persistence: String,
}

fn default_max_work_runs() -> usize { 4 }
fn default_max_task_runs() -> usize { 16 }
fn default_persistence() -> String { "memory-persistence".to_string() }

// ── ThreadWorkRunnerServer ────────────────────────────────────────────────────────

/// Standalone workRunner component.  Wraps a background [`ThreadWorkRunner`] thread
/// and exposes `{name}/Enqueue` and `{name}/Status` ABI actions.
pub struct ThreadWorkRunnerServer {
    handle: Mutex<WorkRunnerHandle>,
    /// Registered component name; used to build action strings.
    name: String,
}

impl ThreadWorkRunnerServer {
    /// Create and immediately start the background workRunner thread.
    pub fn new(cfg: ThreadWorkRunnerConfig, host: Host) -> Self {
        let host_client = HostClient::new(host);
        let workRunner_name = if cfg.name.is_empty() {
            "thread-workRunner".to_string()
        } else {
            cfg.name.clone()
        };
        let persistence = std::sync::Arc::new(
            PersistenceClient::new(host_client.clone(), &cfg.persistence)
        );
        let workRunner_cfg = WorkRunnerConfig {
            name: workRunner_name.clone(),
            kind: "thread".to_string(),
            max_work_runs: cfg.max_work_runs,
            max_task_runs: cfg.max_task_runs,
        };
        let ctx = WorkRunnerContext {
            host:        host_client.clone(),
            persistence,
            workRunner_name: workRunner_name.clone(),
        };
        host_client.log("info", "thread-workRunner",
            &format!("spawning '{}' (max_work_runs={})", workRunner_name, cfg.max_work_runs));
        let handle = ThreadWorkRunner::spawn(&workRunner_cfg, ctx);
        Self { handle: Mutex::new(handle), name: workRunner_name }
    }

    fn enqueue_impl(&self, req: EnqueueRequest) -> Result<EnqueueAck, String> {
        let handle = self.handle.lock().unwrap();
        handle.enqueue(req.work_run_id)
            .map(|_| EnqueueAck { ok: true, queued_to: self.name.clone() })
    }

    fn status_impl(&self) -> WorkRunnerStatusResponse {
        let handle = self.handle.lock().unwrap();
        WorkRunnerStatusResponse {
            name:        self.name.clone(),
            active:      handle.active(),
            capacity:    handle.max_work_runs,
            has_capacity: handle.has_capacity(),
        }
    }
}

// ── PluginComponent impl (manual — dynamic action names) ─────────────────────

impl PluginComponent for ThreadWorkRunnerServer {
    fn get_metadata() -> ComponentMetadata {
        ComponentMetadata {
            kind:        "workaholic/ThreadWorkRunner:1.0".into(),
            name:        "ThreadWorkRunner".into(),
            description: "Executes workflow work runs on a background thread pool.".into(),
        }
    }

    fn to_abi(self) -> orkester_plugin::abi::AbiComponent {
        // Build action names from the workRunner's configured name so multiple
        // workRunner instances can be distinguished in routing.
        let enqueue_action = format!("{}/Enqueue", self.name);
        let status_action = format!("{}/Status", self.name);

        AbiComponentBuilder::new()
            .with_metadata(Self::get_metadata())
            .with_handler(&enqueue_action, |s: &mut Self, req: EnqueueRequest| {
                s.enqueue_impl(req).map_err(|e| -> orkester_plugin::sdk::Error { e.into() })
            })
            .with_handler(&status_action, |s: &mut Self, _req: WorkRunnerStatusRequest| {
                Ok::<_, String>(s.status_impl())
            })
            .build(self)
    }
}
