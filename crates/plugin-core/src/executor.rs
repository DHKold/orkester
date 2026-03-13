use std::collections::HashMap;

use async_trait::async_trait;
use orkester_common::plugin::providers::executor::{
    ExecutionRequest, ExecutionResult, ExecutionStatus, ExecutorBuilder, ExecutorError,
    TaskExecutor,
};
use serde_json::{json, Value};

// ── DummyTaskExecutor ─────────────────────────────────────────────────────────

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

// ── CommandTaskExecutor ───────────────────────────────────────────────────────

/// Runs a local shell command, captures stdout/stderr, and returns them as
/// named outputs.
///
/// # Task config (`TaskSpec.config`)
///
/// ```yaml
/// spec:
///   executor: command
///   config:
///     command: ["echo", "Hello!"]
///     env:                         # optional extra environment variables
///       MY_VAR: "my-value"
///     working_dir: "/tmp"          # optional working directory
/// ```
///
/// # Inputs → environment
///
/// Every input key is upper-cased and `.`/`-` replaced with `_`, then set as
/// an environment variable so commands can read them naturally.
///
/// # Outputs
///
/// | Key         | Type   | Description                              |
/// |-------------|--------|------------------------------------------|
/// | `stdout`    | string | Trimmed standard output of the command.  |
/// | `stderr`    | string | Trimmed standard error output.           |
/// | `exit_code` | number | Numeric exit code returned by the process.|
pub struct CommandTaskExecutor;

#[async_trait]
impl TaskExecutor for CommandTaskExecutor {
    async fn execute(&self, request: ExecutionRequest) -> Result<ExecutionResult, ExecutorError> {
        let cfg = &request.task_definition;

        // ── Parse command array ───────────────────────────────────────────
        let cmd_arr = cfg
            .get("command")
            .and_then(|v| v.as_array())
            .filter(|a| !a.is_empty())
            .ok_or_else(|| {
                ExecutorError::Configuration(
                    "'command' must be a non-empty array of strings".to_string(),
                )
            })?;

        let program = cmd_arr[0]
            .as_str()
            .ok_or_else(|| ExecutorError::Configuration("command[0] must be a string".into()))?;

        let args: Vec<&str> = cmd_arr[1..]
            .iter()
            .map(|v| v.as_str().unwrap_or(""))
            .collect();

        // ── Build the tokio Command ───────────────────────────────────────
        let mut cmd = tokio::process::Command::new(program);
        cmd.args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Optional working directory.
        if let Some(dir) = cfg.get("working_dir").and_then(|v| v.as_str()) {
            cmd.current_dir(dir);
        }

        // Inputs as environment variables (INPUT_KEY=value).
        for (k, v) in &request.inputs {
            let env_key = k.to_uppercase().replace(['.', '-'], "_");
            let env_val = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            cmd.env(env_key, env_val);
        }

        // Extra env from task config (has highest priority, overrides inputs).
        if let Some(env_obj) = cfg.get("env").and_then(|v| v.as_object()) {
            for (k, v) in env_obj {
                if let Some(s) = v.as_str() {
                    cmd.env(k, s);
                }
            }
        }

        // ── Execute ───────────────────────────────────────────────────────
        let output = cmd.output().await.map_err(|e| {
            ExecutorError::Failed(format!("failed to spawn '{}': {}", program, e))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let exit_code = output.status.code().unwrap_or(-1);

        // Collect log lines prefixed by stream.
        let mut logs: Vec<String> = stdout
            .lines()
            .map(|l| format!("[stdout] {l}"))
            .chain(stderr.lines().map(|l| format!("[stderr] {l}")))
            .collect();

        for line in &logs {
            orkester_common::log_info!("{}", line);
        }

        if logs.is_empty() {
            logs.push(format!("[command] exited with code {exit_code}"));
        }

        let mut outputs = HashMap::new();
        outputs.insert("stdout".to_string(), json!(stdout.trim()));
        outputs.insert("stderr".to_string(), json!(stderr.trim()));
        outputs.insert("exit_code".to_string(), json!(exit_code));

        let status = if output.status.success() {
            ExecutionStatus::Succeeded
        } else {
            ExecutionStatus::Failed(format!(
                "command '{}' exited with code {}{}",
                program,
                exit_code,
                if stderr.trim().is_empty() {
                    String::new()
                } else {
                    format!(": {}", stderr.trim())
                }
            ))
        };
        Ok(ExecutionResult { status, outputs, logs })
    }

    async fn cancel(&self, _execution_id: &str) -> Result<(), ExecutorError> {
        // The process has already completed by the time cancel is called.
        Ok(())
    }
}

pub struct CommandExecutorBuilder;

impl ExecutorBuilder for CommandExecutorBuilder {
    fn build(&self, _config: Value) -> Result<Box<dyn TaskExecutor>, ExecutorError> {
        Ok(Box::new(CommandTaskExecutor))
    }
}
