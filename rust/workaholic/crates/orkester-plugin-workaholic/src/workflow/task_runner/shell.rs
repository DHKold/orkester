use std::sync::{Arc, Mutex};

use uuid::Uuid;
use workaholic::{
    TaskRunDoc, TaskRunLogsRef, TaskRunRequestDoc, TaskRunSpec, TaskRunState, TaskRunStatus,
    TaskRunnerDoc, TaskRunnerSpec, TaskRunnerState, TaskRunnerStatus,
};

use super::traits::{TaskRun, TaskRunError, TaskRunEvent, TaskRunEventStream, TaskRunner, TaskRunnerError};

// ─── ShellTaskRunner ───────────────────────────────────────────────────────────────

/// Executes tasks by running a shell script in a child process.
#[derive(Debug)]
pub struct ShellTaskRunner {
    name: String,
    namespace: String,
    spec: TaskRunnerSpec,
    state: Mutex<TaskRunnerState>,
}

impl ShellTaskRunner {
    pub fn new(name: impl Into<String>, namespace: impl Into<String>, spec: TaskRunnerSpec) -> Self {
        Self {
            name: name.into(),
            namespace: namespace.into(),
            spec,
            state: Mutex::new(TaskRunnerState::Ready),
        }
    }

    fn self_ref(&self) -> String {
        format!("worker://{}/{}:1.0.0", self.namespace, self.name)
    }
}

impl TaskRunner for ShellTaskRunner {
    fn as_doc(&self) -> TaskRunnerDoc {
        let state = self.state.lock().unwrap();
        TaskRunnerDoc {
            kind: workaholic::TASK_RUNNER_KIND.to_string(),
            name: self.name.clone(),
            version: "1.0.0".to_string(),
            metadata: workaholic::DocumentMetadata {
                namespace: Some(self.namespace.clone()),
                owner: None,
                description: None,
                tags: vec![],
                extra: Default::default(),
            },
            spec: self.spec.clone(),
            status: Some(TaskRunnerStatus {
                state: state.clone(),
                metrics: Default::default(),
                state_history: vec![],
            }),
        }
    }

    fn spawn(
        &self,
        request: TaskRunRequestDoc,
    ) -> Result<Box<dyn TaskRun>, TaskRunnerError> {
        let run_name = Uuid::new_v4().to_string();
        let run = ShellTaskRun::new(
            run_name,
            self.namespace.clone(),
            self.self_ref(),
            request,
        );
        Ok(Box::new(run))
    }
}

// ─── ShellTaskRun ─────────────────────────────────────────────────────────────────

/// One shell task execution attempt.
#[derive(Debug)]
struct ShellTaskRun {
    name: String,
    namespace: String,
    task_runner_ref: String,
    request: TaskRunRequestDoc,
    /// Shared with the execution thread so both can observe / mutate state.
    state: Arc<Mutex<ShellTaskRunState>>,
    sender: crossbeam_channel::Sender<TaskRunEvent>,
    receiver: crossbeam_channel::Receiver<TaskRunEvent>,
}

#[derive(Debug, Default)]
struct ShellTaskRunState {
    run_state: TaskRunState,
    /// Set to `true` by `cancel()`; the execution thread honours it before
    /// and after the process completes.
    cancel_requested: bool,
    stdout: String,
    stderr: String,
}

impl ShellTaskRun {
    fn new(
        name: String,
        namespace: String,
        task_runner_ref: String,
        request: TaskRunRequestDoc,
    ) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            name,
            namespace,
            task_runner_ref,
            request,
            state: Arc::new(Mutex::new(ShellTaskRunState {
                run_state: TaskRunState::Pending,
                cancel_requested: false,
                stdout: String::new(),
                stderr: String::new(),
            })),
            sender: tx,
            receiver: rx,
        }
    }
}

impl TaskRun for ShellTaskRun {
    fn as_doc(&self) -> TaskRunDoc {
        let state = self.state.lock().unwrap();
        TaskRunDoc {
            kind: workaholic::TASK_RUN_KIND.to_string(),
            name: self.name.clone(),
            version: "1.0.0".to_string(),
            metadata: workaholic::DocumentMetadata {
                namespace: Some(self.namespace.clone()),
                owner: None,
                description: None,
                tags: vec![],
                extra: Default::default(),
            },
            spec: TaskRunSpec {
                task_run_request_ref: self.request.name.clone(),
                work_run_ref: String::new(),
                work_ref: self.request.spec.work_ref.clone(),
                task_ref: self.request.spec.task_ref.clone(),
                step_name: self.request.spec.step_name.clone(),
                attempt: 1,
                work_runner_ref: String::new(),
                task_runner_ref: self.task_runner_ref.clone(),
            },
            status: Some(TaskRunStatus {
                state: state.run_state.clone(),
                created_at: None,
                started_at: None,
                finished_at: None,
                outputs: {
                    let total = self.request.spec.outputs.len();
                    self.request.spec.outputs.iter()
                        .filter_map(|o| {
                            extract_output_value(&o.name, &state.stdout, total)
                                .map(|v| (o.name.clone(), v))
                        })
                        .collect()
                },
                inputs: self.request.spec.inputs.iter().map(|i| {
                    let val = match &i.from {
                        workaholic::TaskInputSource::Literal { value } => value.clone(),
                        workaholic::TaskInputSource::ArtifactRef { uri } => serde_json::Value::String(uri.clone()),
                    };
                    (i.name.clone(), val)
                }).collect(),
                state_history: vec![],
                logs_ref: if !state.stdout.is_empty() || !state.stderr.is_empty() {
                    Some(TaskRunLogsRef {
                        stdout: state.stdout.clone(),
                        stderr: state.stderr.clone(),
                    })
                } else {
                    None
                },
            }),
        }
    }

    fn start(&self) -> Result<(), TaskRunError> {
        {
            let mut g = self.state.lock().unwrap();
            if g.run_state != TaskRunState::Pending {
                return Err(TaskRunError::AlreadyStarted);
            }
            g.run_state = TaskRunState::Running;
        }
        let _ = self.sender.send(TaskRunEvent::StateChanged(TaskRunState::Running));

        // Extract the shell script and env vars from the resolved request.
        let script = self
            .request
            .spec
            .execution
            .config
            .get("script")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let env_vars: Vec<(String, String)> = self
            .request
            .spec
            .inputs
            .iter()
            .map(|i| {
                let val = match &i.from {
                    workaholic::TaskInputSource::Literal { value } => {
                        value.as_str().map(|s| s.to_string()).unwrap_or_else(|| value.to_string())
                    }
                    workaholic::TaskInputSource::ArtifactRef { uri } => uri.clone(),
                };
                (i.name.clone(), val)
            })
            .collect();

        // Spawn the script on a background thread; we share `state` and `sender`
        // so the thread can update the run state and emit events when done.
        let shared_state = Arc::clone(&self.state);
        let sender = self.sender.clone();

        std::thread::spawn(move || {
            // Bail immediately if cancellation was requested before we got scheduled.
            if shared_state.lock().unwrap().cancel_requested {
                let _ = sender.send(TaskRunEvent::Finished);
                return;
            }

            let mut cmd = std::process::Command::new("sh");
            cmd.arg("-c").arg(&script);
            for (k, v) in &env_vars {
                cmd.env(k, v);
            }

            let final_state = match cmd.output() {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    if !stdout.is_empty() { eprintln!("[shell] stdout:\n{stdout}"); }
                    if !stderr.is_empty() { eprintln!("[shell] stderr:\n{stderr}"); }
                    let mut g = shared_state.lock().unwrap();
                    g.stdout = stdout;
                    g.stderr = stderr;
                    if g.cancel_requested {
                        g.run_state = TaskRunState::Cancelled;
                        TaskRunState::Cancelled
                    } else if output.status.success() {
                        g.run_state = TaskRunState::Succeeded;
                        TaskRunState::Succeeded
                    } else {
                        let code = output.status.code().unwrap_or(-1);
                        log::warn!(
                            "Shell task '{}' exited with non-zero code {}",
                            script.lines().next().unwrap_or("<empty>"),
                            code
                        );
                        g.run_state = TaskRunState::Failed;
                        TaskRunState::Failed
                    }
                }
                Err(e) => {
                    log::error!("Failed to spawn shell process: {}", e);
                    let mut g = shared_state.lock().unwrap();
                    g.run_state = TaskRunState::Failed;
                    TaskRunState::Failed
                }
            };

            let _ = sender.send(TaskRunEvent::StateChanged(final_state));
            let _ = sender.send(TaskRunEvent::Finished);
        });

        Ok(())
    }

    fn cancel(&self) -> Result<(), TaskRunError> {
        let mut g = self.state.lock().unwrap();
        if matches!(
            g.run_state,
            TaskRunState::Succeeded | TaskRunState::Failed | TaskRunState::Cancelled
        ) {
            return Err(TaskRunError::AlreadyFinished);
        }
        // Signal the execution thread to abort as soon as it checks the flag.
        g.cancel_requested = true;
        g.run_state = TaskRunState::Cancelled;
        drop(g);
        let _ = self.sender.send(TaskRunEvent::StateChanged(TaskRunState::Cancelled));
        let _ = self.sender.send(TaskRunEvent::Finished);
        Ok(())
    }

    fn subscribe(&self) -> TaskRunEventStream {
        use super::stream_adapter::CrossbeamStream;
        Box::pin(CrossbeamStream::new(self.receiver.clone()))
    }
}

// ─── Output extraction ────────────────────────────────────────────────────────

/// Extract a named output value from shell stdout.
///
/// Tries two conventions in order:
/// 1. A line of the form `<name>=<value>`.
/// 2. If `total_outputs == 1`, the last non-empty trimmed line.
fn extract_output_value(name: &str, stdout: &str, total_outputs: usize) -> Option<serde_json::Value> {
    let prefix = format!("{name}=");
    for line in stdout.lines() {
        if let Some(val) = line.trim().strip_prefix(&prefix) {
            return Some(serde_json::Value::String(val.to_string()));
        }
    }
    if total_outputs == 1 {
        stdout.lines().filter(|l| !l.trim().is_empty()).last()
            .map(|l| serde_json::Value::String(l.trim().to_string()))
    } else {
        None
    }
}