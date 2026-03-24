pub mod thread;
pub mod thread_worker_server;

pub use thread::ThreadWorker;
pub use thread_worker_server::{ThreadWorkerConfig, ThreadWorkerServer};

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use workaholic::traits::PersistenceProvider;

use crate::host_client::HostClient;

// ── WorkerHandle ──────────────────────────────────────────────────────────────

/// Owner-side handle to a background worker.
pub struct WorkerHandle {
    pub name: String,
    pub kind: String,
    pub sender: crossbeam_channel::Sender<String>,
    pub active_count: Arc<AtomicUsize>,
    pub max_work_runs: usize,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl WorkerHandle {
    pub fn active(&self) -> usize {
        self.active_count.load(Ordering::Relaxed)
    }

    pub fn has_capacity(&self) -> bool {
        self.active() < self.max_work_runs
    }

    pub fn enqueue(&self, work_run_id: String) -> Result<(), String> {
        self.sender
            .try_send(work_run_id)
            .map_err(|e| format!("worker '{}' queue full: {e}", self.name))
    }
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

// ── WorkerContext ─────────────────────────────────────────────────────────────

/// Data shared between the WorkflowServer and worker background threads.
pub struct WorkerContext {
    pub host: HostClient,
    pub persistence: Arc<dyn PersistenceProvider>,
    pub worker_name: String,
}

// ── WorkerConfig ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct WorkerConfig {
    pub name: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default = "default_max_work_runs")]
    pub max_work_runs: usize,
    #[serde(default = "default_max_task_runs")]
    pub max_task_runs: usize,
}

fn default_kind() -> String { "thread".to_string() }
fn default_max_work_runs() -> usize { 4 }
fn default_max_task_runs() -> usize { 16 }
