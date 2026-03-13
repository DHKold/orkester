use std::collections::HashMap;
use std::sync::Arc;

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
    /// Named outputs produced by the execution (passed to downstream steps).
    pub outputs: HashMap<String, Value>,
    /// Human-readable log lines captured during execution.
    pub logs: Vec<String>,
}

/// A task execution request passed to an executor.
#[derive(Debug, Clone)]
pub struct ExecutionRequest {
    /// Unique execution ID (UUID v4 stamped by the worker).
    pub id: String,
    /// Executor-specific configuration taken verbatim from `TaskSpec.config`.
    pub task_definition: Value,
    /// Resolved runtime inputs (merged workflow context + step overrides).
    pub inputs: HashMap<String, Value>,
    /// Outputs to capture from the execution and pass to downstream steps (executor-specific).
    pub outputs: Vec<String>,
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

// ── ExecutorRegistry ──────────────────────────────────────────────────────────

/// Runtime registry of named [`TaskExecutor`] instances.
///
/// Built once at startup from the loaded plugin components and shared
/// (via [`Arc`]) with every server that needs to dispatch tasks.
#[derive(Default)]
pub struct ExecutorRegistry {
    executors: HashMap<String, Arc<dyn TaskExecutor>>,
}

impl ExecutorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an executor under `name` (e.g. `"command"`, `"eks-pod"`).
    pub fn register(&mut self, name: impl Into<String>, executor: Arc<dyn TaskExecutor>) {
        self.executors.insert(name.into(), executor);
    }

    /// Look up a registered executor by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn TaskExecutor>> {
        self.executors.get(name).cloned()
    }
}
