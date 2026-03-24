use std::sync::Arc;

use workaholic::traits::PersistenceProvider;

use crate::{host_client::HostClient, worker::WorkerHandle};

/// Shared state between the WorkflowServer component and its worker threads.
pub struct WorkflowServerState {
    pub persistence: Arc<dyn PersistenceProvider>,
    pub host: HostClient,
    pub default_namespace: String,
    /// Worker handles (and their background threads).
    pub workers: Vec<WorkerHandle>,
    /// Round-robin counter used for worker selection.
    pub next_worker: usize,
}

impl WorkflowServerState {
    /// Select the worker with the most available capacity.
    /// Falls back to round-robin if all workers are at capacity.
    pub fn pick_worker(&mut self) -> Option<&WorkerHandle> {
        if self.workers.is_empty() {
            return None;
        }
        // Prefer a worker that has spare capacity.
        if let Some(w) = self.workers.iter().find(|w| w.has_capacity()) {
            return Some(w);
        }
        // All at capacity — still submit via round-robin (queue will buffer).
        let idx = self.next_worker % self.workers.len();
        self.next_worker = self.next_worker.wrapping_add(1);
        Some(&self.workers[idx])
    }
}
