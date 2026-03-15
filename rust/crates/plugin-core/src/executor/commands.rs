use std::collections::HashMap;

use async_trait::async_trait;
use orkester_common::plugin::providers::executor::{
    ExecutionRequest, ExecutionResult, ExecutionStatus, ExecutorBuilder, ExecutorError,
    TaskExecutor,
};
use serde_json::{json, Value};

/// Runs a sequence of shell commands as a **single `sh -c` invocation**, so
/// all commands share the same shell process — `export VAR=value` in one
/// command is visible to every subsequent command.  Execution stops at the
/// first non-zero exit code (`set -e` is prepended automatically).
///
/// # Task config (`TaskSpec.config`)
///
/// ```yaml
/// spec:
///   executor: commands
///   config:
///     commands:
///       - ["echo", "Hello from Orkester!"]   # array form: each part is shell-quoted
///       - export MY_VAR=hello                # string form: shell built-ins work!
///       - ["echo", "Done!"]
///     env:                                   # optional static environment variables
///       MY_VAR: "my-value"
///     working_dir: "/tmp"                    # optional working directory
/// ```
///
/// # Inputs → environment
///
/// Every input key is upper-cased and `.`/`-` replaced with `_`, then set as
/// an environment variable for the shell process.  Commands can read and
/// override them freely using normal shell syntax (`export VAR=value`).
///
/// # Outputs
///
/// After all commands complete the executor appends a sentinel line followed
/// by `KEY=VALUE` pairs for every name in `ExecutionRequest.outputs`.  Those
/// lines are parsed as outputs and stripped from the visible logs.
///
/// | Key          | Type   | Description                                                    |
/// |--------------|--------|----------------------------------------------------------------|
/// | `$?`         | number | Exit code of the shell process.                                |
/// | `<OUTPUT>`   | string | Each name listed in `ExecutionRequest.outputs`, read from the  |
/// |              |        | shell environment at the moment the output section executes.   |
pub struct CommandsTaskExecutor;

/// Magic line printed by the generated script to mark the start of the
/// key=value output section in stdout.
const OUTPUTS_SENTINEL: &str = "___ORKESTER_OUTPUTS___";

// ── CommandEntry ─────────────────────────────────────────────────────────────

/// Parsed form of a single entry in the `commands` list.
enum CommandEntry {
    /// Direct exec: the first element is the program, the rest are arguments.
    Array(Vec<String>),
    /// Shell invocation: run via `sh -c "<string>"`.
    Shell(String),
}

impl CommandEntry {
    fn from_value(v: &Value) -> Result<Self, ExecutorError> {
        match v {
            Value::Array(arr) => {
                if arr.is_empty() {
                    return Err(ExecutorError::Configuration(
                        "command array entry must not be empty".to_string(),
                    ));
                }
                let parts = arr
                    .iter()
                    .map(|s| {
                        s.as_str().map(str::to_owned).ok_or_else(|| {
                            ExecutorError::Configuration(
                                "command array elements must be strings".to_string(),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(CommandEntry::Array(parts))
            }
            Value::String(s) => Ok(CommandEntry::Shell(s.clone())),
            _ => Err(ExecutorError::Configuration(
                "each command must be a string or an array of strings".to_string(),
            )),
        }
    }

    /// Convert to a shell-script line suitable for embedding in a `sh -c` script.
    fn into_shell_line(self) -> String {
        match self {
            CommandEntry::Array(parts) => {
                parts.iter().map(|p| shell_quote(p)).collect::<Vec<_>>().join(" ")
            }
            CommandEntry::Shell(s) => s,
        }
    }
}

// ── Shell quoting ─────────────────────────────────────────────────────────────

/// Wrap `s` in single quotes, escaping any embedded single quotes as `'\''`.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''" ))
}

// ── RunContext ────────────────────────────────────────────────────────────────

/// Shared context derived from the task config and execution inputs.
struct RunContext {
    commands:    Vec<CommandEntry>,
    env:         Vec<(String, String)>,
    working_dir: Option<String>,
}

impl RunContext {
    fn build(cfg: &Value, inputs: &HashMap<String, Value>) -> Result<Self, ExecutorError> {
        let commands = Self::parse_commands(cfg)?;
        let env      = Self::build_env(cfg, inputs);
        let working_dir = cfg.get("working_dir").and_then(|v| v.as_str()).map(str::to_owned);
        Ok(RunContext { commands, env, working_dir })
    }

    fn parse_commands(cfg: &Value) -> Result<Vec<CommandEntry>, ExecutorError> {
        let raw = cfg
            .get("commands")
            .and_then(|v| v.as_array())
            .filter(|a| !a.is_empty())
            .ok_or_else(|| {
                ExecutorError::Configuration(
                    "'commands' must be a non-empty list of strings or arrays".to_string(),
                )
            })?;
        raw.iter().map(CommandEntry::from_value).collect()
    }

    /// Build the environment variable list.
    ///
    /// Priority (lowest → highest):
    /// 1. Inputs: each key is upper-cased, `.`/`-` replaced with `_`.
    /// 2. `config.env` static overrides.
    fn build_env(cfg: &Value, inputs: &HashMap<String, Value>) -> Vec<(String, String)> {
        let mut env: Vec<(String, String)> = inputs
            .iter()
            .map(|(k, v)| {
                let key = k.to_uppercase().replace(['.', '-'], "_");
                let val = match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                (key, val)
            })
            .collect();

        if let Some(env_obj) = cfg.get("env").and_then(|v| v.as_object()) {
            for (k, v) in env_obj {
                if let Some(s) = v.as_str() {
                    env.push((k.clone(), s.to_owned()));
                }
            }
        }

        env
    }
}

// ── TaskExecutor impl ─────────────────────────────────────────────────────────

#[async_trait]
impl TaskExecutor for CommandsTaskExecutor {
    async fn execute(&self, request: ExecutionRequest) -> Result<ExecutionResult, ExecutorError> {
        let ctx = RunContext::build(&request.task_definition, &request.inputs)?;

        tracing::debug!(
            execution_id  = %request.id,
            command_count = ctx.commands.len(),
            working_dir   = ?ctx.working_dir,
            env_keys      = ?ctx.env.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
            "CommandsTaskExecutor: starting execution"
        );

        let RunContext { commands, env, working_dir } = ctx;
        let script = Self::build_shell_script(commands, &request.outputs);

        tracing::debug!(execution_id = %request.id, %script, "generated shell script");

        let output = Self::spawn_shell(&script, &env, working_dir.as_deref()).await?;

        let stdout    = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr    = String::from_utf8_lossy(&output.stderr).into_owned();
        let exit_code = output.status.code().unwrap_or(-1);

        tracing::debug!(
            execution_id = %request.id,
            exit_code,
            stdout = stdout.trim(),
            stderr = stderr.trim(),
            "shell script finished"
        );

        let (outputs, log_stdout) = Self::parse_outputs(&stdout, &request.outputs, exit_code);

        let mut logs: Vec<String> = log_stdout
            .lines()
            .map(|l| format!("[stdout] {l}"))
            .chain(stderr.lines().map(|l| format!("[stderr] {l}")))
            .collect();

        if logs.is_empty() {
            logs.push(format!("[command] script exited with code {exit_code}"));
        }

        for line in &logs {
            orkester_common::log_info!("{}", line);
        }

        let status = if output.status.success() {
            ExecutionStatus::Succeeded
        } else {
            let msg = Self::failure_message(exit_code, stderr.trim());
            tracing::warn!(execution_id = %request.id, %msg, "script failed");
            ExecutionStatus::Failed(msg)
        };

        Ok(ExecutionResult { status, outputs, logs })
    }

    async fn cancel(&self, _execution_id: &str) -> Result<(), ExecutorError> {
        Ok(())
    }
}

impl CommandsTaskExecutor {
    /// Build the shell script that runs all commands followed by an output-dump
    /// section for the declared output variable names.
    ///
    /// `set -e` is prepended so the script aborts on the first non-zero exit.
    /// `set +e` precedes the output section so a missing variable does not
    /// prevent the remaining outputs from being printed.
    fn build_shell_script(commands: Vec<CommandEntry>, declared_outputs: &[String]) -> String {
        let mut lines = vec!["set -e".to_string()];
        lines.extend(commands.into_iter().map(CommandEntry::into_shell_line));

        if !declared_outputs.is_empty() {
            lines.push("set +e".to_string());
            lines.push(format!("echo '{OUTPUTS_SENTINEL}'"));
            for name in declared_outputs {
                lines.push(format!("printf '%s=%s\\n' '{name}' \"${name}\""));
            }
        }

        lines.join("\n")
    }

    async fn spawn_shell(
        script: &str,
        env: &[(String, String)],
        working_dir: Option<&str>,
    ) -> Result<std::process::Output, ExecutorError> {
        let mut cmd = tokio::process::Command::new("sh");
        cmd.args(["-c", script]);
        cmd.stdout(std::process::Stdio::piped())
           .stderr(std::process::Stdio::piped());

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }
        for (k, v) in env {
            cmd.env(k, v);
        }

        cmd.output()
            .await
            .map_err(|e| ExecutorError::Failed(format!("failed to spawn shell: {e}")))
    }

    /// Split `stdout` at the sentinel line.
    ///
    /// Lines before the sentinel go to logs; lines after are parsed as
    /// `KEY=VALUE` pairs and inserted into the outputs map.  `$?` is always
    /// set to `exit_code` regardless of whether the sentinel is present.
    fn parse_outputs(
        stdout: &str,
        declared_outputs: &[String],
        exit_code: i32,
    ) -> (HashMap<String, Value>, String) {
        let mut outputs: HashMap<String, Value> = HashMap::new();
        outputs.insert("$?".to_string(), json!(exit_code));

        let lines: Vec<&str> = stdout.lines().collect();
        let sentinel_pos     = lines.iter().position(|l| *l == OUTPUTS_SENTINEL);

        let log_stdout = match sentinel_pos {
            None => stdout.to_string(),
            Some(pos) => {
                for line in &lines[pos + 1..] {
                    if let Some((k, v)) = line.split_once('=') {
                        if declared_outputs.iter().any(|o| o == k) {
                            outputs.insert(k.to_string(), json!(v));
                        }
                    }
                }
                lines[..pos].join("\n")
            }
        };

        (outputs, log_stdout)
    }

    fn failure_message(exit_code: i32, stderr: &str) -> String {
        if stderr.is_empty() {
            format!("script exited with code {exit_code}")
        } else {
            format!("script exited with code {exit_code}: {stderr}")
        }
    }
}

// ── Builder ───────────────────────────────────────────────────────────────────

pub struct CommandsExecutorBuilder;

impl ExecutorBuilder for CommandsExecutorBuilder {
    fn build(&self, _config: Value) -> Result<Box<dyn TaskExecutor>, ExecutorError> {
        Ok(Box::new(CommandsTaskExecutor))
    }
}
