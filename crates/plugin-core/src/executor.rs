use async_trait::async_trait;
use orkester_common::plugin::providers::executor::
    {ExecutionRequest, ExecutionResult, ExecutionStatus, ExecutorBuilder, ExecutorError, TaskExecutor};
use serde_json::{json, Value};

/// A dummy task executor that logs the task and immediately returns success.
/// Useful for testing workflows without real execution backends.
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
            artifacts: json!({}),
            logs: vec![format!(
                "[dummy] Task '{}' executed successfully (no-op)",
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
