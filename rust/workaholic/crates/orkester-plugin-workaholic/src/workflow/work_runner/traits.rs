use std::pin::Pin;

use futures_core::Stream;
use workaholic::{TaskRunDoc, WorkRunDoc, WorkRunRequestDoc, WorkRunnerDoc};

// ─── Resource grants ──────────────────────────────────────────────────────────

/// Consumable resource tokens that the `WorkRunner` grants to a `WorkRun`.
/// A `WorkRun` must return unused tokens immediately via `grant()`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkRunResources {
    /// Number of task execution permits granted for this call.
    pub task_permits: usize,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum WorkRunnerError {
    #[error("WorkRun not found: {0}")]
    NotFound(String),
    #[error("Capacity exceeded: {0}")]
    CapacityExceeded(String),
    #[error("WorkRunner is not active")]
    NotActive,
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, thiserror::Error)]
pub enum WorkRunError {
    #[error("Already started")]
    AlreadyStarted,
    #[error("Already finished")]
    AlreadyFinished,
    #[error("Cancelled")]
    Cancelled,
    #[error("Task run error: {0}")]
    TaskRunError(String),
    #[error("{0}")]
    Other(String),
}

// ─── Events ───────────────────────────────────────────────────────────────────

/// Events emitted by a `WorkRun` during its lifecycle.
#[derive(Debug, Clone)]
pub enum WorkRunEvent {
    /// The WorkRun transitioned to a new top-level state.
    StateChanged(workaholic::WorkRunState),
    /// A step transitioned to a new state.
    StepStateChanged {
        step_name: String,
        state: workaholic::WorkRunState,
    },
    /// A new TaskRun was spawned for a step.
    TaskRunCreated {
        step_name: String,
        task_run_ref: String,
    },
    /// A TaskRun's state changed (forwarded from the TaskRun's own events).
    TaskRunUpdated {
        step_name: String,
        task_run_ref: String,
        state: workaholic::TaskRunState,
    },
    /// The WorkRun completed (succeeded, failed, or cancelled).
    Finished,
}

/// Async stream of `WorkRunEvent`s.  Callers receive this from `WorkRun::subscribe()`.
pub type WorkRunEventStream = Pin<Box<dyn Stream<Item = WorkRunEvent> + Send>>;

// ─── Traits ───────────────────────────────────────────────────────────────────

/// Global workflow execution engine.  Manages resource allocation, scheduling,
/// and the lifecycle of all active `WorkRun`s.
pub trait WorkRunner: Send + Sync + std::fmt::Debug {
    /// Export a document snapshot of this runner's current state.
    fn as_doc(&self) -> WorkRunnerDoc;

    /// Create a new `WorkRun` from a frozen request.
    ///
    /// Does not start execution; call `WorkRun::start()` explicitly.
    fn spawn(
        &self,
        request: WorkRunRequestDoc,
    ) -> Result<Box<dyn WorkRun>, WorkRunnerError>;
}

/// Live orchestrator for one workflow execution.  Owns the DAG state and
/// starts `TaskRun`s using task permits granted by the `WorkRunner`.
pub trait WorkRun: Send + Sync + std::fmt::Debug {
    /// Export a document snapshot of this run's current state.
    fn as_doc(&self) -> WorkRunDoc;

    /// Begin execution.  Must be called once after `WorkRunner::spawn()`.
    fn start(&self) -> Result<(), WorkRunError>;

    /// Request cancellation of all in-progress steps and the overall run.
    fn cancel(&self) -> Result<(), WorkRunError>;

    /// Grant consumable task permits to this run.
    ///
    /// The run consumes as many permits as it currently needs and returns
    /// any it cannot use right now so the `WorkRunner` can reallocate them.
    fn grant(&self, resources: WorkRunResources) -> Result<WorkRunResources, WorkRunError>;

    /// Subscribe to events emitted by this run.
    fn subscribe(&self) -> WorkRunEventStream;

    /// Deliver an updated document from a `TaskRun` that this run spawned.
    fn on_task_run_update(&self, step_name: &str, task_run: TaskRunDoc);
}

/// Handle to a spawned `TaskRunRequest`, carried inside a `WorkRun` as a
/// reference to the underlying `TaskRunner`-managed execution.
pub trait TaskRunHandle: Send + Sync + std::fmt::Debug {
    fn as_doc(&self) -> TaskRunDoc;
    fn start(&self) -> Result<(), workaholic::WorkaholicError>;
    fn cancel(&self) -> Result<(), workaholic::WorkaholicError>;
}
