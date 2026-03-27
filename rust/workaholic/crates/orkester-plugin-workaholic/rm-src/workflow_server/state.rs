use std::sync::Arc;

use workaholic::traits::PersistenceProvider;

use crate::{host_client::HostClient, workRunner::WorkRunnerHandle};

/// Shared state between the WorkflowServer component and its workRunner threads.
pub struct WorkflowServerState {
    pub persistence: Arc<dyn PersistenceProvider>,
    pub host: HostClient,
    pub default_namespace: String,
    /// WorkRunner handles (and their background threads).
    pub workRunners: Vec<WorkRunnerHandle>,
    /// Round-robin counter used for workRunner selection.
    pub next_workRunner: usize,
}

impl WorkflowServerState {
    /// Select the workRunner with the most available capacity.
    /// Falls back to round-robin if all workRunners are at capacity.
    pub fn pick_workRunner(&mut self) -> Option<&WorkRunnerHandle> {
        if self.workRunners.is_empty() {
            return None;
        }
        // Prefer a workRunner that has spare capacity.
        if let Some(w) = self.workRunners.iter().find(|w| w.has_capacity()) {
            return Some(w);
        }
        // All at capacity — still submit via round-robin (queue will buffer).
        let idx = self.next_workRunner % self.workRunners.len();
        self.next_workRunner = self.next_workRunner.wrapping_add(1);
        Some(&self.workRunners[idx])
    }
}
