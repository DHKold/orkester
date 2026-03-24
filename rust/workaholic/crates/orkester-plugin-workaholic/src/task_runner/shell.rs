use std::{
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};

use crossbeam_channel::{Receiver, Sender};
use workaholic::execution::task_run::TaskRunPhase;

use super::{TaskRunEvent, TaskRunHandle, TaskRunResult, TaskRunner};

// ── ShellTaskRunner ───────────────────────────────────────────────────────────

/// Executes tasks via a local shell process.
///
/// Expects the resolved `inputs` JSON to contain either:
/// * `"script"` — a shell script string executed with `sh -c`
/// * `"command"` — an array of strings with the first element as the program
///
/// Optional fields: `"cwd"`, `"env"` (object), `"args"` (extra CLI args),
/// `"timeout_seconds"`.
pub struct ShellTaskRunner;

impl ShellTaskRunner {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ShellTaskRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskRunner for ShellTaskRunner {
    fn kind(&self) -> &'static str {
        "shell"
    }

    fn spawn(&mut self, task_name: &str, inputs: &serde_json::Value) -> Box<dyn TaskRunHandle> {
        let inputs = inputs.clone();
        let task_name = task_name.to_string();

        let (result_tx, result_rx) = crossbeam_channel::bounded::<TaskRunResult>(1);
        let (cancel_tx, cancel_rx) = crossbeam_channel::bounded::<()>(1);
        let (event_tx, event_rx) = crossbeam_channel::unbounded::<TaskRunEvent>();

        let phase = Arc::new(Mutex::new(TaskRunPhase::Starting));
        let phase_clone = phase.clone();
        let event_tx_clone = event_tx.clone();

        std::thread::Builder::new()
            .name(format!("shell-task-{task_name}"))
            .spawn(move || {
                set_phase(&phase_clone, &event_tx_clone, TaskRunPhase::Running);

                let result = execute_shell(&inputs, &cancel_rx, &event_tx_clone);

                set_phase(&phase_clone, &event_tx_clone, result.phase.clone());
                let _ = result_tx.send(result);
                // Closing event channel signals end-of-stream to subscribers.
            })
            .expect("failed to spawn shell task thread");

        Box::new(ShellTaskRunHandle { phase, result_rx, cancel_tx, event_rx })
    }
}

// ── ShellTaskRunHandle ────────────────────────────────────────────────────────

pub struct ShellTaskRunHandle {
    phase: Arc<Mutex<TaskRunPhase>>,
    result_rx: Receiver<TaskRunResult>,
    cancel_tx: Sender<()>,
    event_rx: Receiver<TaskRunEvent>,
}

impl TaskRunHandle for ShellTaskRunHandle {
    fn status(&self) -> TaskRunPhase {
        self.phase.lock().map(|p| p.clone()).unwrap_or(TaskRunPhase::Running)
    }

    fn cancel(&self) -> workaholic::Result<()> {
        let _ = self.cancel_tx.try_send(());
        Ok(())
    }

    fn wait(self: Box<Self>) -> TaskRunResult {
        match self.result_rx.recv() {
            Ok(r) => r,
            Err(_) => TaskRunResult::failed("CHANNEL_CLOSED", "task runner thread disappeared"),
        }
    }

    fn subscribe(&self) -> Receiver<TaskRunEvent> {
        self.event_rx.clone()
    }
}

// ── Execution logic ───────────────────────────────────────────────────────────

fn set_phase(
    phase: &Arc<Mutex<TaskRunPhase>>,
    events: &Sender<TaskRunEvent>,
    new_phase: TaskRunPhase,
) {
    if let Ok(mut p) = phase.lock() {
        *p = new_phase.clone();
    }
    let _ = events.send(TaskRunEvent::PhaseChanged(new_phase));
}

fn execute_shell(
    inputs: &serde_json::Value,
    cancel_rx: &Receiver<()>,
    events: &Sender<TaskRunEvent>,
) -> TaskRunResult {
    let timeout = Duration::from_secs(inputs["timeout_seconds"].as_u64().unwrap_or(3600));
    let cwd = inputs["cwd"].as_str().map(str::to_owned);

    let mut cmd = if let Some(script) = inputs["script"].as_str() {
        let mut c = Command::new("sh");
        c.arg("-c").arg(script);
        c
    } else if let Some(arr) = inputs["command"].as_array() {
        let parts: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
        if parts.is_empty() {
            return TaskRunResult::failed("INVALID_COMMAND", "empty command array");
        }
        let extra_args: Vec<String> = inputs["args"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_owned())).collect())
            .unwrap_or_default();
        let mut c = Command::new(parts[0]);
        c.args(&parts[1..]).args(&extra_args);
        c
    } else {
        return TaskRunResult::failed(
            "MISSING_EXECUTION_CONFIG",
            "execution config must have 'script' or 'command'",
        );
    };

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(env_map) = inputs["env"].as_object() {
        for (k, v) in env_map {
            if let Some(val) = v.as_str() {
                cmd.env(k, val);
            }
        }
    }
    if let Some(ref dir) = cwd {
        cmd.current_dir(dir);
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return TaskRunResult::failed("SPAWN_FAILED", e.to_string()),
    };

    let start = std::time::Instant::now();

    loop {
        // Check for cancellation request.
        if cancel_rx.try_recv().is_ok() {
            let _ = child.kill();
            return TaskRunResult::failed("CANCELLED", "task cancelled by request");
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                use std::io::Read;
                let stdout = child
                    .stdout
                    .take()
                    .and_then(|mut o| {
                        let mut s = String::new();
                        o.read_to_string(&mut s).ok()?;
                        Some(s)
                    })
                    .unwrap_or_default();

                if !stdout.is_empty() {
                    let _ = events.send(TaskRunEvent::LogLine {
                        level: "info".into(),
                        message: stdout.clone(),
                    });
                }

                return if status.success() {
                    TaskRunResult::succeeded(serde_json::json!({ "stdout": stdout }))
                } else {
                    let code = status.code().unwrap_or(-1).to_string();
                    TaskRunResult::failed(
                        format!("EXIT_{code}"),
                        format!("process exited with code {code}"),
                    )
                };
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return TaskRunResult::failed("TIMEOUT", "task execution timed out");
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return TaskRunResult::failed("WAIT_FAILED", e.to_string()),
        }
    }
}
