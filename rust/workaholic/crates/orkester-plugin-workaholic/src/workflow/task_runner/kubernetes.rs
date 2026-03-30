//! Kubernetes task runner — executes tasks as Kubernetes Jobs.
//!
//! The runner calls `kubectl` to create a `Job`, then polls its status until
//! it reaches a terminal state.  All resolved task inputs are injected as
//! environment variables into the job's container.
//!
//! # Runner config keys (inside `spec.config`)
//!
//! | Key            | Type   | Description                                               |
//! |----------------|--------|-----------------------------------------------------------|
//! | `image`        | string | Container image reference (required).                    |
//! | `namespace`    | string | Kubernetes namespace (default: `"default"`).             |
//! | `context`      | string | kubectl context to use (optional, uses current by default). |
//! | `poll_secs`    | u64    | Status polling interval in seconds (default: 5).         |
//! | `timeout_secs` | u64    | Hard deadline for the entire job (default: 3600).        |
//! | `service_account` | string | ServiceAccount to bind to the job pod (optional).    |
//! | `command`      | array  | Entrypoint command override (optional).                  |
//! | `args`         | array  | Arguments override (optional).                           |

use std::sync::{Arc, Mutex};
use std::time::Duration;

use uuid::Uuid;
use workaholic::{
    DocumentMetadata, TaskRunDoc, TaskRunRequestDoc, TaskRunSpec, TaskRunState, TaskRunStatus,
    TaskRunnerDoc, TaskRunnerSpec, TaskRunnerState, TaskRunnerStatus, TASK_RUN_KIND,
    TASK_RUNNER_KIND,
};

use super::traits::{TaskRun, TaskRunError, TaskRunEvent, TaskRunEventStream, TaskRunner, TaskRunnerError};
use super::stream_adapter::CrossbeamStream;

// ─── KubernetesTaskRunner ─────────────────────────────────────────────────────

/// Executes tasks as Kubernetes Jobs using the `kubectl` CLI.
#[derive(Debug)]
pub struct KubernetesTaskRunner {
    name:      String,
    namespace: String,
    spec:      TaskRunnerSpec,
    state:     Mutex<TaskRunnerState>,
}

impl KubernetesTaskRunner {
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

impl TaskRunner for KubernetesTaskRunner {
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
        let cfg = &request.spec.execution.config;

        let image = cfg.get("image").and_then(|v| v.as_str())
            .ok_or_else(|| TaskRunnerError::Other("missing 'image' in kubernetes runner config".into()))?
            .to_string();
        let kube_ns   = cfg.get("namespace").and_then(|v| v.as_str()).unwrap_or("default").to_string();
        let context   = cfg.get("context").and_then(|v| v.as_str()).map(|s| s.to_string());
        let poll_secs = cfg.get("poll_secs").and_then(|v| v.as_u64()).unwrap_or(5);
        let timeout   = cfg.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(3600);
        let svc_acc   = cfg.get("service_account").and_then(|v| v.as_str()).map(|s| s.to_string());
        let command: Vec<String> = cfg.get("command").and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let args: Vec<String> = cfg.get("args").and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();

        let run = KubernetesTaskRun::new(
            Uuid::new_v4().to_string(), self.namespace.clone(), self.self_ref(),
            request, image, kube_ns, context, poll_secs, timeout, svc_acc, command, args,
        );
        Ok(Box::new(run))
    }
}

// ─── KubernetesTaskRun ────────────────────────────────────────────────────────

#[derive(Debug)]
struct KubernetesTaskRun {
    name:             String,
    namespace:        String,
    task_runner_ref:  String,
    request:          TaskRunRequestDoc,
    image:            String,
    kube_namespace:   String,
    context:          Option<String>,
    poll_secs:        u64,
    timeout_secs:     u64,
    service_account:  Option<String>,
    command:          Vec<String>,
    args:             Vec<String>,
    state:            Arc<Mutex<KubernetesTaskRunState>>,
    sender:           crossbeam_channel::Sender<TaskRunEvent>,
    receiver:         crossbeam_channel::Receiver<TaskRunEvent>,
}

#[derive(Debug, Default)]
struct KubernetesTaskRunState {
    run_state:        TaskRunState,
    cancel_requested: bool,
    job_name:         Option<String>,
}

impl KubernetesTaskRun {
    #[allow(clippy::too_many_arguments)]
    fn new(
        name: String, namespace: String, task_runner_ref: String,
        request: TaskRunRequestDoc, image: String, kube_namespace: String,
        context: Option<String>, poll_secs: u64, timeout_secs: u64,
        service_account: Option<String>, command: Vec<String>, args: Vec<String>,
    ) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            name, namespace, task_runner_ref, request,
            image, kube_namespace, context, poll_secs, timeout_secs,
            service_account, command, args,
            state: Arc::new(Mutex::new(KubernetesTaskRunState {
                run_state: TaskRunState::Pending, cancel_requested: false, job_name: None,
            })),
            sender: tx, receiver: rx,
        }
    }
}

impl TaskRun for KubernetesTaskRun {
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
                outputs: Default::default(), inputs: Default::default(),
                state_history: vec![], logs_ref: None,
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
        let cfg = KubeJobConfig {
            image:           self.image.clone(),
            kube_namespace:  self.kube_namespace.clone(),
            context:         self.context.clone(),
            poll_secs:       self.poll_secs,
            timeout_secs:    self.timeout_secs,
            service_account: self.service_account.clone(),
            command:         self.command.clone(),
            args:            self.args.clone(),
            env_vars,
        };

        std::thread::spawn(move || run_kubernetes_job(cfg, shared, sender));
        Ok(())
    }

    fn cancel(&self) -> Result<(), TaskRunError> {
        let job_name = {
            let mut g = self.state.lock().unwrap();
            if matches!(g.run_state, TaskRunState::Succeeded | TaskRunState::Failed | TaskRunState::Cancelled) {
                return Err(TaskRunError::AlreadyFinished);
            }
            g.cancel_requested = true;
            g.run_state = TaskRunState::Cancelled;
            g.job_name.clone()
        };
        if let Some(jn) = job_name {
            // Best-effort Job deletion.
            let _ = std::process::Command::new("kubectl")
                .args(["delete", "job", &jn, "--ignore-not-found"])
                .output();
        }
        let _ = self.sender.send(TaskRunEvent::StateChanged(TaskRunState::Cancelled));
        let _ = self.sender.send(TaskRunEvent::Finished);
        Ok(())
    }

    fn subscribe(&self) -> TaskRunEventStream {
        Box::pin(CrossbeamStream::new(self.receiver.clone()))
    }
}

// ─── Kubernetes helpers ───────────────────────────────────────────────────────

struct KubeJobConfig {
    image:           String,
    kube_namespace:  String,
    context:         Option<String>,
    poll_secs:       u64,
    timeout_secs:    u64,
    service_account: Option<String>,
    command:         Vec<String>,
    args:            Vec<String>,
    env_vars:        Vec<(String, String)>,
}

fn collect_env_vars(request: &TaskRunRequestDoc) -> Vec<(String, String)> {
    request.spec.inputs.iter().map(|i| {
        let val = match &i.from {
            workaholic::TaskInputSource::Literal   { value } =>
                value.as_str().map(|s| s.to_string()).unwrap_or_else(|| value.to_string()),
            workaholic::TaskInputSource::ArtifactRef { uri } => uri.clone(),
        };
        (i.name.clone(), val)
    }).collect()
}

fn run_kubernetes_job(
    cfg:    KubeJobConfig,
    state:  Arc<Mutex<KubernetesTaskRunState>>,
    sender: crossbeam_channel::Sender<TaskRunEvent>,
) {
    if state.lock().unwrap().cancel_requested {
        let _ = sender.send(TaskRunEvent::Finished);
        return;
    }

    let job_name = format!("orkester-{}", Uuid::new_v4().to_string().split('-').next().unwrap_or("job"));
    let manifest = build_job_manifest(&job_name, &cfg);

    if let Err(e) = apply_kubectl_manifest(&manifest, &cfg.context) {
        log::error!("[k8s runner] failed to create Job '{}': {}", job_name, e);
        state.lock().unwrap().run_state = TaskRunState::Failed;
        let _ = sender.send(TaskRunEvent::StateChanged(TaskRunState::Failed));
        let _ = sender.send(TaskRunEvent::Finished);
        return;
    }

    state.lock().unwrap().job_name = Some(job_name.clone());

    let deadline  = std::time::Instant::now() + Duration::from_secs(cfg.timeout_secs);
    let poll_dur  = Duration::from_secs(cfg.poll_secs);

    let final_state = poll_job_until_done(
        &job_name, &cfg.kube_namespace, &cfg.context, deadline, poll_dur, &state,
    );

    delete_job(&job_name, &cfg.kube_namespace, &cfg.context);
    state.lock().unwrap().run_state = final_state.clone();
    let _ = sender.send(TaskRunEvent::StateChanged(final_state));
    let _ = sender.send(TaskRunEvent::Finished);
}

fn build_job_manifest(job_name: &str, cfg: &KubeJobConfig) -> String {
    let env_yaml = cfg.env_vars.iter()
        .map(|(k, v)| format!("        - name: {}\n          value: {:?}", k, v))
        .collect::<Vec<_>>()
        .join("\n");

    let cmd_yaml = if cfg.command.is_empty() {
        String::new()
    } else {
        let items = cfg.command.iter().map(|s| format!("          - {:?}", s)).collect::<Vec<_>>().join("\n");
        format!("        command:\n{}\n", items)
    };

    let args_yaml = if cfg.args.is_empty() {
        String::new()
    } else {
        let items = cfg.args.iter().map(|s| format!("          - {:?}", s)).collect::<Vec<_>>().join("\n");
        format!("        args:\n{}\n", items)
    };

    let svc_acc = cfg.service_account.as_deref().map(|s| {
        format!("      serviceAccountName: {}\n", s)
    }).unwrap_or_default();

    format!(
        "apiVersion: batch/v1\nkind: Job\nmetadata:\n  name: {job}\n  namespace: {ns}\n\
         spec:\n  template:\n    spec:\n{sa}      restartPolicy: Never\n      containers:\n\
               - name: task\n        image: {img}\n{cmd}{args}        env:\n{env}\n",
        job = job_name,
        ns  = cfg.kube_namespace,
        sa  = svc_acc,
        img = cfg.image,
        cmd = cmd_yaml,
        args = args_yaml,
        env = env_yaml,
    )
}

fn apply_kubectl_manifest(yaml: &str, context: &Option<String>) -> Result<(), String> {
    let mut cmd = std::process::Command::new("kubectl");
    if let Some(ctx) = context { cmd.args(["--context", ctx]); }
    cmd.args(["apply", "-f", "-"]);
    use std::io::Write;
    let mut child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("kubectl spawn: {}", e))?;
    child.stdin.as_mut().unwrap().write_all(yaml.as_bytes()).map_err(|e| e.to_string())?;
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() { Ok(()) } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

fn poll_job_until_done(
    job_name:  &str,
    kube_ns:   &str,
    context:   &Option<String>,
    deadline:  std::time::Instant,
    poll_dur:  Duration,
    state:     &Arc<Mutex<KubernetesTaskRunState>>,
) -> TaskRunState {
    loop {
        if state.lock().unwrap().cancel_requested {
            return TaskRunState::Cancelled;
        }
        if std::time::Instant::now() >= deadline {
            log::warn!("[k8s runner] job '{}' timed out", job_name);
            return TaskRunState::Failed;
        }
        match query_job_status(job_name, kube_ns, context) {
            Ok(Some(final_state)) => return final_state,
            Ok(None)              => {}
            Err(e) => log::warn!("[k8s runner] status query error: {}", e),
        }
        std::thread::sleep(poll_dur);
    }
}

fn query_job_status(
    job_name: &str,
    kube_ns:  &str,
    context:  &Option<String>,
) -> Result<Option<TaskRunState>, String> {
    let mut cmd = std::process::Command::new("kubectl");
    if let Some(ctx) = context { cmd.args(["--context", ctx]); }
    cmd.args(["get", "job", job_name, "-n", kube_ns,
              "-o", "jsonpath={.status.conditions[*].type}/{.status.conditions[*].status}"]);
    let out = cmd.output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout);
    if text.contains("Complete/True") { return Ok(Some(TaskRunState::Succeeded)); }
    if text.contains("Failed/True")   { return Ok(Some(TaskRunState::Failed)); }
    Ok(None)
}

fn delete_job(job_name: &str, kube_ns: &str, context: &Option<String>) {
    let mut cmd = std::process::Command::new("kubectl");
    if let Some(ctx) = context { cmd.args(["--context", ctx]); }
    cmd.args(["delete", "job", job_name, "-n", kube_ns, "--ignore-not-found"]);
    let _ = cmd.output();
}
