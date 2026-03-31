//! Container task runner — executes tasks inside an OCI container.
//!
//! The runner invokes the Docker or Podman CLI to pull and run a container
//! image, passing resolved task inputs as environment variables.  The run
//! completes when the container exits.
//!
//! # Runner config keys (inside `spec.config`)
//!
//! | Key        | Type   | Description                                                  |
//! |------------|--------|--------------------------------------------------------------|
//! | `image`    | string | OCI image reference (required).                             |
//! | `command`  | string | Entrypoint override (optional).                             |
//! | `args`     | array  | Argument list override (optional).                          |
//! | `runtime`  | string | Container runtime binary: `"docker"` or `"podman"` (default `"docker"`). |
//! | `rm`       | bool   | Remove container after exit (default `true`).               |

use std::sync::{Arc, Mutex};

use uuid::Uuid;
use workaholic::{
    DocumentMetadata, TaskRunDoc, TaskRunLogsRef, TaskRunRequestDoc, TaskRunSpec, TaskRunState,
    TaskRunStatus, TaskRunnerDoc, TaskRunnerSpec, TaskRunnerState, TaskRunnerStatus, TASK_RUN_KIND,
    TASK_RUNNER_KIND,
};
use orkester_plugin::{log_debug, log_error, log_warn};

use super::traits::{TaskRun, TaskRunError, TaskRunEvent, TaskRunEventStream, TaskRunner, TaskRunnerError};
use super::stream_adapter::CrossbeamStream;

// ─── ContainerTaskRunner ──────────────────────────────────────────────────────

/// Executes tasks inside an OCI container via the Docker or Podman CLI.
#[derive(Debug)]
pub struct ContainerTaskRunner {
    name:      String,
    namespace: String,
    spec:      TaskRunnerSpec,
    state:     Mutex<TaskRunnerState>,
}

impl ContainerTaskRunner {
    pub fn new(
        name:      impl Into<String>,
        namespace: impl Into<String>,
        spec:      TaskRunnerSpec,
    ) -> Self {
        Self {
            name:      name.into(),
            namespace: namespace.into(),
            spec,
            state: Mutex::new(TaskRunnerState::Ready),
        }
    }

    fn self_ref(&self) -> String {
        format!("worker://{}/{}:1.0.0", self.namespace, self.name)
    }
}

impl TaskRunner for ContainerTaskRunner {
    fn as_doc(&self) -> TaskRunnerDoc {
        let state = self.state.lock().unwrap();
        TaskRunnerDoc {
            kind:     TASK_RUNNER_KIND.to_string(),
            name:     self.name.clone(),
            version:  "1.0.0".to_string(),
            metadata: DocumentMetadata {
                namespace: Some(self.namespace.clone()),
                owner: None, description: None, tags: vec![], extra: Default::default(),
            },
            spec:   self.spec.clone(),
            status: Some(TaskRunnerStatus {
                state:         state.clone(),
                metrics:       Default::default(),
                state_history: vec![],
            }),
        }
    }

    fn spawn(&self, request: TaskRunRequestDoc) -> Result<Box<dyn TaskRun>, TaskRunnerError> {
        let image = request
            .spec.execution.config.get("image")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TaskRunnerError::Other("missing 'image' in container runner config".into()))?
            .to_string();

        let runtime = request.spec.execution.config
            .get("runtime").and_then(|v| v.as_str())
            .unwrap_or("docker").to_string();
        let command = request.spec.execution.config
            .get("command").and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let args: Vec<String> = request.spec.execution.config
            .get("args").and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let rm = request.spec.execution.config
            .get("rm").and_then(|v| v.as_bool()).unwrap_or(true);

        let run = ContainerTaskRun::new(
            Uuid::new_v4().to_string(), self.namespace.clone(), self.self_ref(),
            request, image, runtime, command, args, rm,
        );
        Ok(Box::new(run))
    }
}

// ─── ContainerTaskRun ─────────────────────────────────────────────────────────

#[derive(Debug)]
struct ContainerTaskRun {
    name:             String,
    namespace:        String,
    task_runner_ref:  String,
    request:          TaskRunRequestDoc,
    image:            String,
    runtime:          String,
    command:          Option<String>,
    args:             Vec<String>,
    rm:               bool,
    state:            Arc<Mutex<ContainerTaskRunState>>,
    sender:           crossbeam_channel::Sender<TaskRunEvent>,
    receiver:         crossbeam_channel::Receiver<TaskRunEvent>,
}

#[derive(Debug, Default)]
struct ContainerTaskRunState {
    run_state:        TaskRunState,
    cancel_requested: bool,
    container_id:     Option<String>,
    stdout:           String,
    stderr:           String,
}

impl ContainerTaskRun {
    #[allow(clippy::too_many_arguments)]
    fn new(
        name: String, namespace: String, task_runner_ref: String,
        request: TaskRunRequestDoc, image: String, runtime: String,
        command: Option<String>, args: Vec<String>, rm: bool,
    ) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            name, namespace, task_runner_ref, request,
            image, runtime, command, args, rm,
            state: Arc::new(Mutex::new(ContainerTaskRunState {
                run_state: TaskRunState::Pending, cancel_requested: false, container_id: None,
                stdout: String::new(), stderr: String::new(),
            })),
            sender: tx, receiver: rx,
        }
    }
}

impl TaskRun for ContainerTaskRun {
    fn as_doc(&self) -> TaskRunDoc {
        let state = self.state.lock().unwrap();
        TaskRunDoc {
            kind: TASK_RUN_KIND.to_string(), name: self.name.clone(), version: "1.0.0".to_string(),
            metadata: DocumentMetadata {
                namespace: Some(self.namespace.clone()),
                owner: None, description: None, tags: vec![], extra: Default::default(),
            },
            spec: TaskRunSpec {
                task_run_request_ref: self.request.name.clone(),
                work_run_ref: String::new(), work_ref: self.request.spec.work_ref.clone(),
                task_ref: self.request.spec.task_ref.clone(),
                step_name: self.request.spec.step_name.clone(),
                attempt: 1, work_runner_ref: String::new(),
                task_runner_ref: self.task_runner_ref.clone(),
            },
            status: Some(TaskRunStatus {
                state: state.run_state.clone(),
                created_at: None, started_at: None, finished_at: None,
                outputs: Default::default(),
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

        let env_vars = collect_env_vars(&self.request);
        let shared   = Arc::clone(&self.state);
        let sender   = self.sender.clone();
        let runtime  = self.runtime.clone();
        let image    = self.image.clone();
        let command  = self.command.clone();
        let args     = self.args.clone();
        let rm       = self.rm;

        std::thread::spawn(move || {
            run_container(runtime, image, command, args, env_vars, rm, shared, sender);
        });
        Ok(())
    }

    fn cancel(&self) -> Result<(), TaskRunError> {
        let container_id = {
            let mut g = self.state.lock().unwrap();
            if matches!(g.run_state, TaskRunState::Succeeded | TaskRunState::Failed | TaskRunState::Cancelled) {
                return Err(TaskRunError::AlreadyFinished);
            }
            g.cancel_requested = true;
            g.run_state = TaskRunState::Cancelled;
            g.container_id.clone()
        };
        if let Some(cid) = container_id {
            let _ = std::process::Command::new("docker").args(["stop", &cid]).output();
        }
        let _ = self.sender.send(TaskRunEvent::StateChanged(TaskRunState::Cancelled));
        let _ = self.sender.send(TaskRunEvent::Finished);
        Ok(())
    }

    fn subscribe(&self) -> TaskRunEventStream {
        Box::pin(CrossbeamStream::new(self.receiver.clone()))
    }
}

// ─── Container helpers ────────────────────────────────────────────────────────

fn collect_env_vars(request: &TaskRunRequestDoc) -> Vec<(String, String)> {
    request.spec.inputs.iter().map(|i| {
        let val = match &i.from {
            workaholic::TaskInputSource::Literal   { value } => {
                value.as_str().map(|s| s.to_string()).unwrap_or_else(|| value.to_string())
            }
            workaholic::TaskInputSource::ArtifactRef { uri } => uri.clone(),
        };
        (i.name.clone(), val)
    }).collect()
}

fn run_container(
    runtime:   String,
    image:     String,
    command:   Option<String>,
    extra_args: Vec<String>,
    env_vars:  Vec<(String, String)>,
    rm:        bool,
    state:     Arc<Mutex<ContainerTaskRunState>>,
    sender:    crossbeam_channel::Sender<TaskRunEvent>,
) {
    if state.lock().unwrap().cancel_requested {
        let _ = sender.send(TaskRunEvent::Finished);
        return;
    }

    // Build arguments: run [--rm] [-e K=V ...] image [command] [args...]
    let mut cmd_args: Vec<String> = vec!["run".into(), "--name".into(),
        format!("orkester-{}", uuid::Uuid::new_v4())];
    if rm { cmd_args.push("--rm".into()); }
    for (k, v) in &env_vars {
        cmd_args.push("-e".into());
        cmd_args.push(format!("{}={}", k, v));
    }
    cmd_args.push(image);
    if let Some(cmd) = command { cmd_args.push(cmd); }
    cmd_args.extend(extra_args);

    let final_state = match std::process::Command::new(&runtime).args(&cmd_args).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if !stdout.is_empty() { log_debug!("[container] stdout:\n{stdout}"); }
            if !stderr.is_empty() { log_debug!("[container] stderr:\n{stderr}"); }
            let mut g = state.lock().unwrap();
            g.stdout = stdout;
            g.stderr = stderr;
            if g.cancel_requested {
                g.run_state = TaskRunState::Cancelled;
                TaskRunState::Cancelled
            } else if output.status.success() {
                g.run_state = TaskRunState::Succeeded;
                TaskRunState::Succeeded
            } else {
                log_warn!("[container runner] exited with code {:?}", output.status.code());
                g.run_state = TaskRunState::Failed;
                TaskRunState::Failed
            }
        }
        Err(e) => {
            log_error!("[container runner] failed to spawn '{}': {}", runtime, e);
            let mut g = state.lock().unwrap();
            g.stderr    = format!("Failed to spawn container runtime '{}': {}", runtime, e);
            g.run_state = TaskRunState::Failed;
            TaskRunState::Failed
        }
    };

    let _ = sender.send(TaskRunEvent::StateChanged(final_state));
    let _ = sender.send(TaskRunEvent::Finished);
}
