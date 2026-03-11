use crate::domain::{ExecutionId, Work, WorkExecution, WorkExecutionStatus};
use crate::plugin::servers::ServerContext;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("Execution not found: {0}")]
    NotFound(String),
    #[error("Invalid work definition: {0}")]
    InvalidWork(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// A request to execute a Work.
#[derive(Debug, Clone)]
pub struct ExecutionRequest {
    pub work: Work,
    /// Optional input artifacts / parameters (key → value).
    pub inputs: Value,
}

/// Interface for submitting and monitoring Work executions.
/// The REST server and services interact with the engine through this handle.
#[async_trait]
pub trait WorkflowHandle: Send + Sync {
    /// Submit a Work for execution. Returns the execution ID immediately.
    async fn submit(&self, request: ExecutionRequest) -> Result<ExecutionId, WorkflowError>;

    /// Request cancellation of a running execution.
    async fn cancel(&self, id: &ExecutionId) -> Result<(), WorkflowError>;

    /// Get the current state of an execution.
    async fn status(&self, id: &ExecutionId) -> Result<WorkExecution, WorkflowError>;

    /// List all executions (optionally filtered by status).
    async fn list(
        &self,
        status_filter: Option<WorkExecutionStatus>,
    ) -> Result<Vec<WorkExecution>, WorkflowError>;
}

/// A running Workflow (execution engine) server.
#[async_trait]
pub trait WorkflowServer: Send + Sync {
    fn name(&self) -> &str;
    fn handle(&self) -> Arc<dyn WorkflowHandle>;
    fn run(self: Box<Self>) -> ServerContext<ExecutionRequest, ()>;
}
