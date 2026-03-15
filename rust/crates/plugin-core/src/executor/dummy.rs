use std::collections::HashMap;

use async_trait::async_trait;
use orkester_common::plugin::providers::executor::{
    ExecutionRequest, ExecutionResult, ExecutionStatus, ExecutorBuilder, ExecutorError,
    TaskExecutor,
};
use serde_json::Value;

/// A no-op executor that immediately returns success. Useful for testing.
pub struct DummyTaskExecutor;

#[async_trait]
impl TaskExecutor for DummyTaskExecutor {
    async fn execute(&self, request: ExecutionRequest) -> Result<ExecutionResult, ExecutorError> {
        tracing::info!(
            execution_id = %request.id,
            task = ?request.task_definition,
            "DummyTaskExecutor: executing task (no-op)"
        );
        Ok(ExecutionResult {
            status: ExecutionStatus::Succeeded,
            outputs: HashMap::new(),
            logs: vec![format!(
                "[dummy] Execution '{}' completed as no-op",
                request.id
            )],
        })
    }

    async fn cancel(&self, execution_id: &str) -> Result<(), ExecutorError> {
        tracing::info!(execution_id = %execution_id, "DummyTaskExecutor: cancel (no-op)");
        Ok(())
    }
}

pub struct DummyExecutorBuilder;

impl ExecutorBuilder for DummyExecutorBuilder {
    fn build(&self, _config: Value) -> Result<Box<dyn TaskExecutor>, ExecutorError> {
        Ok(Box::new(DummyTaskExecutor))
    }
}
