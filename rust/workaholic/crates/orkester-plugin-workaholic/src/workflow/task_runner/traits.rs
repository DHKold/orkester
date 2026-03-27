use std::pin::Pin;

use futures_core::Stream;
use workaholic::{TaskRunDoc, TaskRunRequestDoc, TaskRunnerDoc};

// ─── Errors ───────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum TaskRunnerError {
    #[error("Not ready")]
    NotReady,
    #[error("Unsupported runner kind: {0}")]
    UnsupportedKind(String),
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, thiserror::Error)]
pub enum TaskRunError {
    #[error("Already started")]
    AlreadyStarted,
    #[error("Already finished")]
    AlreadyFinished,
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
    #[error("Cancelled")]
    Cancelled,
    #[error("{0}")]
    Other(String),
}

// ─── Events ───────────────────────────────────────────────────────────────────

/// Events emitted by a `TaskRun` during its lifecycle.
#[derive(Debug, Clone)]
pub enum TaskRunEvent {
    /// The TaskRun transitioned to a new state.
    StateChanged(workaholic::TaskRunState),
    /// A named output value was produced or updated.
    OutputUpdated { output: String },
    /// The TaskRun completed (succeeded, failed, or cancelled).
    Finished,
}

/// Async stream of `TaskRunEvent`s.  Callers receive this from `TaskRun::subscribe()`.
pub type TaskRunEventStream = Pin<Box<dyn Stream<Item = TaskRunEvent> + Send>>;

// ─── Traits ───────────────────────────────────────────────────────────────────

/// Execution backend for tasks.  Each backend (shell, container, SQL, …)
/// implements its own `TaskRunner`.
pub trait TaskRunner: Send + Sync + std::fmt::Debug {
    /// Export a document snapshot of this runner's current state.
    fn as_doc(&self) -> TaskRunnerDoc;

    /// Create a new `TaskRun` from a frozen request.
    ///
    /// Does not start execution; call `TaskRun::start()` explicitly.
    fn spawn(
        &self,
        request: TaskRunRequestDoc,
    ) -> Result<Box<dyn TaskRun>, TaskRunnerError>;
}

/// One concrete task execution attempt.  Created from a `TaskRunRequest` by a
/// `TaskRunner`; a retry creates a new `TaskRun` from the same frozen request.
pub trait TaskRun: Send + Sync + std::fmt::Debug {
    /// Export a document snapshot of this run's current state.
    fn as_doc(&self) -> TaskRunDoc;

    /// Begin execution.  Must be called once after `TaskRunner::spawn()`.
    fn start(&self) -> Result<(), TaskRunError>;

    /// Request cancellation of the underlying execution.
    fn cancel(&self) -> Result<(), TaskRunError>;

    /// Subscribe to events emitted by this run.
    fn subscribe(&self) -> TaskRunEventStream;
}
