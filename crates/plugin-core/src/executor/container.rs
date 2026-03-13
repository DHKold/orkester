use async_trait::async_trait;
use orkester_common::plugin::providers::executor::{
    ExecutionRequest, ExecutionResult, ExecutorBuilder, ExecutorError, TaskExecutor,
};
use serde_json::Value;

/// Runs a task inside an OCI container (Podman / Docker).
///
/// # Task config (`TaskSpec.config`)
///
/// ```yaml
/// spec:
///   executor: container
///   config:
///     image: "alpine:3.19"
///     command: ["sh", "-c", "echo hello"]   # optional override of the image entrypoint
///     env:                                  # optional extra environment variables
///       MY_VAR: "my-value"
///     working_dir: "/work"                  # optional working directory inside the container
///     pull_policy: "if-not-present"         # always | never | if-not-present (default)
/// ```
///
/// # Inputs → environment
///
/// Every input key is upper-cased and `.`/`-` replaced with `_`, then injected
/// as an environment variable inside the container.
///
/// # Outputs
///
/// | Key         | Type   | Description                               |
/// |-------------|--------|-------------------------------------------|
/// | `stdout`    | string | Trimmed standard output of the container. |
/// | `stderr`    | string | Trimmed standard error output.            |
/// | `exit_code` | number | Numeric exit code returned by the process.|
pub struct ContainerTaskExecutor;

#[async_trait]
impl TaskExecutor for ContainerTaskExecutor {
    async fn execute(&self, _request: ExecutionRequest) -> Result<ExecutionResult, ExecutorError> {
        // TODO: implement container execution
        //
        // Suggested steps:
        //   1. Parse `_request.task_definition` for `image`, `command`, `env`,
        //      `working_dir`, `pull_policy`.
        //   2. Detect runtime (podman / docker) or read from executor config.
        //   3. Pull the image according to `pull_policy`.
        //   4. Build `tokio::process::Command` for `podman run --rm ...` / `docker run --rm ...`.
        //   5. Inject inputs as `-e KEY=value` flags.
        //   6. Stream stdout/stderr, collect logs.
        //   7. Map exit code → ExecutionStatus::Succeeded / Failed.
        Err(ExecutorError::Failed(
            "ContainerTaskExecutor is not yet implemented".to_string(),
        ))
    }

    async fn cancel(&self, _execution_id: &str) -> Result<(), ExecutorError> {
        // TODO: issue `podman stop <container-id>` / `docker stop <container-id>`
        Ok(())
    }
}

pub struct ContainerExecutorBuilder;

impl ExecutorBuilder for ContainerExecutorBuilder {
    fn build(&self, _config: Value) -> Result<Box<dyn TaskExecutor>, ExecutorError> {
        Ok(Box::new(ContainerTaskExecutor))
    }
}
