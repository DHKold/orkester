pub mod container;
pub mod kubernetes;
pub mod shell;

pub use container::ContainerTaskRunner;
pub use kubernetes::KubernetesTaskRunner;
pub use shell::ShellTaskRunner;

use workaholic::{
    domain::task::ExecutionKind,
    execution::task_run::{TaskRunError, TaskRunPhase},
};

// ── TaskRunResult ─────────────────────────────────────────────────────────────

/// Final outcome of a task execution.
#[derive(Debug)]
pub struct TaskRunResult {
    pub phase: TaskRunPhase,
    pub outputs: serde_json::Value,
    pub external_id: Option<String>,
    pub error: Option<TaskRunError>,
}

impl TaskRunResult {
    pub fn succeeded(outputs: serde_json::Value) -> Self {
        Self { phase: TaskRunPhase::Succeeded, outputs, external_id: None, error: None }
    }

    pub fn failed(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            phase: TaskRunPhase::Failed,
            outputs: serde_json::Value::Null,
            external_id: None,
            error: Some(TaskRunError { code: code.into(), message: message.into() }),
        }
    }
}

// ── TaskRunEvent ──────────────────────────────────────────────────────────────

/// Streaming event emitted by a running task.
#[derive(Debug, Clone)]
pub enum TaskRunEvent {
    /// The task's execution phase changed.
    PhaseChanged(TaskRunPhase),
    /// A log line was produced by the task.
    LogLine { level: String, message: String },
    /// The external execution ID became known (e.g. PID, K8s Job name).
    ExternalId(String),
}

// ── TaskRunHandle ─────────────────────────────────────────────────────────────

/// Live handle to a spawned task execution.
pub trait TaskRunHandle: Send {
    /// Current execution phase (non-blocking snapshot).
    fn status(&self) -> TaskRunPhase;

    /// Request cancellation of the running task.  Best-effort; may not take
    /// effect immediately for external executors.
    fn cancel(&self) -> workaholic::Result<()>;

    /// Block the calling thread until the task completes and return the result.
    fn wait(self: Box<Self>) -> TaskRunResult;

    /// Subscribe to task events.  The receiver is cloneable and may be held
    /// across threads.  The channel is closed once the task terminates.
    fn subscribe(&self) -> crossbeam_channel::Receiver<TaskRunEvent>;
}

// ── TaskRunner ────────────────────────────────────────────────────────────────

/// Abstraction over different execution backends.
///
/// `spawn` is non-blocking: it starts the task and returns a [`TaskRunHandle`]
/// immediately.  The caller then uses `handle.wait()` to block until completion
/// or `handle.subscribe()` to receive streaming events.
pub trait TaskRunner: Send {
    /// Short human-readable kind identifier (for log messages).
    fn kind(&self) -> &'static str;

    /// Start executing the task described by `inputs` (the resolved, merged
    /// execution config field from the task spec + work-task overrides).
    fn spawn(
        &mut self,
        task_name: &str,
        inputs: &serde_json::Value,
    ) -> Box<dyn TaskRunHandle>;
}

// ── Factory ───────────────────────────────────────────────────────────────────

/// Construct the appropriate task runner for a task based on its execution kind.
pub fn build_runner(kind: &ExecutionKind) -> Box<dyn TaskRunner> {
    match kind {
        ExecutionKind::Shell => Box::new(ShellTaskRunner::new()),
        ExecutionKind::Container => Box::new(ContainerTaskRunner::new()),
        ExecutionKind::Kubernetes => Box::new(KubernetesTaskRunner::new()),
        _ => {
            log::warn!("[task_runner] unknown execution kind {kind:?}, falling back to shell");
            Box::new(ShellTaskRunner::new())
        }
    }
}
