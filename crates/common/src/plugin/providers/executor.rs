use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("Execution failed: {0}")]
    Failed(String),
    #[error("Configuration error: {0}")]
    Configuration(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// The current status of a task execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionStatus {
    Pending,
    Running,
    Succeeded,
    Failed(String),
    Cancelled,
}

/// The result of executing a task.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub status: ExecutionStatus,
    /// Artifacts produced by the execution, as key-value pairs.
    pub artifacts: Value,
    /// Logs produced during execution.
    pub logs: Vec<String>,
}

/// A task execution request passed to an executor.
#[derive(Debug, Clone)]
pub struct ExecutionRequest {
    /// Unique execution ID.
    pub id: String,
    /// Task definition (executor-specific fields).
    pub task_definition: Value,
    /// Input artifacts/parameters.
    pub inputs: Value,
}

/// Trait that all Task Executor implementations must satisfy.
///
/// Executors abstract over execution backends: shell, Kubernetes, Podman, etc.
#[async_trait]
pub trait TaskExecutor: Send + Sync {
    /// Execute the given task and return the result.
    async fn execute(&self, request: ExecutionRequest) -> Result<ExecutionResult, ExecutorError>;

    /// Cancel a running execution by ID.
    async fn cancel(&self, execution_id: &str) -> Result<(), ExecutorError>;
}

/// Builder that creates a [`TaskExecutor`] from a JSON configuration.
pub trait ExecutorBuilder: Send + Sync {
    fn build(&self, config: Value) -> Result<Box<dyn TaskExecutor>, ExecutorError>;
}
