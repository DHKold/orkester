use super::{TaskRunHandle, TaskRunResult, TaskRunner};
use crate::task_runner::container::StubHandle;

/// Submits tasks as Kubernetes Jobs.
///
/// Not yet implemented — returns a failed result immediately.
/// Reserved for future implementation via the Kubernetes API.
pub struct KubernetesTaskRunner;

impl KubernetesTaskRunner {
    pub fn new() -> Self { Self }
}

impl Default for KubernetesTaskRunner {
    fn default() -> Self { Self::new() }
}

impl TaskRunner for KubernetesTaskRunner {
    fn kind(&self) -> &'static str { "kubernetes" }

    fn spawn(&mut self, _task_name: &str, _inputs: &serde_json::Value) -> Box<dyn TaskRunHandle> {
        let (result_tx, result_rx) = crossbeam_channel::bounded(1);
        let (_, event_rx) = crossbeam_channel::unbounded();
        let _ = result_tx.send(TaskRunResult::failed(
            "NOT_IMPLEMENTED",
            "KubernetesTaskRunner is not yet implemented",
        ));
        Box::new(StubHandle { result_rx, event_rx })
    }
}
