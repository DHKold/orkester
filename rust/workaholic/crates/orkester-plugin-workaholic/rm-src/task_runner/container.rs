use super::{TaskRunHandle, TaskRunResult, TaskRunner};

/// Executes tasks by running a local or remote container.
///
/// Not yet implemented — falls back to returning a failed result immediately.
/// Reserved for future implementation using the Docker SDK or similar.
pub struct ContainerTaskRunner;

impl ContainerTaskRunner {
    pub fn new() -> Self { Self }
}

impl Default for ContainerTaskRunner {
    fn default() -> Self { Self::new() }
}

impl TaskRunner for ContainerTaskRunner {
    fn kind(&self) -> &'static str { "container" }

    fn spawn(&mut self, _task_name: &str, _inputs: &serde_json::Value) -> Box<dyn TaskRunHandle> {
        // Return a pre-resolved failed handle.
        let (result_tx, result_rx) = crossbeam_channel::bounded(1);
        let (_, event_rx) = crossbeam_channel::unbounded();
        let _ = result_tx.send(TaskRunResult::failed(
            "NOT_IMPLEMENTED",
            "ContainerTaskRunner is not yet implemented",
        ));
        Box::new(StubHandle { result_rx, event_rx })
    }
}

pub(super) struct StubHandle {
    pub result_rx: crossbeam_channel::Receiver<TaskRunResult>,
    pub event_rx: crossbeam_channel::Receiver<super::TaskRunEvent>,
}

impl TaskRunHandle for StubHandle {
    fn status(&self) -> workaholic::execution::task_run::TaskRunPhase {
        workaholic::execution::task_run::TaskRunPhase::Failed
    }
    fn cancel(&self) -> workaholic::Result<()> { Ok(()) }
    fn wait(self: Box<Self>) -> TaskRunResult {
        self.result_rx.recv().unwrap_or_else(|_| TaskRunResult::failed("LOST", "channel closed"))
    }
    fn subscribe(&self) -> crossbeam_channel::Receiver<super::TaskRunEvent> {
        self.event_rx.clone()
    }
}
