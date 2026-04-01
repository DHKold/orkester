use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use kube::Client;
use orkester_plugin::log_warn;
use serde_json::Value;
use workaholic::{
    DocumentMetadata, TaskInputSource, TaskRunDoc, TaskRunLogsRef, TaskRunRequestDoc,
    TaskRunSpec, TaskRunState, TaskRunStatus, TASK_RUN_KIND,
};

use super::super::stream_adapter::CrossbeamStream;
use super::super::traits::{TaskRun, TaskRunError, TaskRunEvent, TaskRunEventStream};
use super::config::KubeJobConfig;
use super::exec::run_kubernetes_job;
use super::job::delete_job;
use super::outputs::extract_output_value;
use super::state::KubeTaskRunState;

#[derive(Debug)]
pub struct KubernetesTaskRun {
    name:            String,
    namespace:       String,
    task_runner_ref: String,
    request:         TaskRunRequestDoc,
    cfg:             KubeJobConfig,
    pub state:       Arc<Mutex<KubeTaskRunState>>,
    sender:          crossbeam_channel::Sender<TaskRunEvent>,
    receiver:        crossbeam_channel::Receiver<TaskRunEvent>,
}

impl KubernetesTaskRun {
    pub fn new(
        name: String, namespace: String, task_runner_ref: String,
        request: TaskRunRequestDoc, cfg: KubeJobConfig,
    ) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            name, namespace, task_runner_ref, request, cfg,
            state: Arc::new(Mutex::new(KubeTaskRunState::pending())),
            sender: tx,
            receiver: rx,
        }
    }

    fn build_outputs(&self, stdout: &str) -> HashMap<String, Value> {
        let total = self.request.spec.outputs.len();
        self.request.spec.outputs.iter()
            .filter_map(|o| extract_output_value(&o.name, stdout, total).map(|v| (o.name.clone(), v)))
            .collect()
    }

    fn build_inputs(&self) -> HashMap<String, Value> {
        self.request.spec.inputs.iter()
            .map(|i| {
                let v = match &i.from {
                    TaskInputSource::Literal { value } => value.clone(),
                    TaskInputSource::ArtifactRef { uri } => Value::String(uri.clone()),
                };
                (i.name.clone(), v)
            })
            .collect()
    }
}

impl TaskRun for KubernetesTaskRun {
    fn as_doc(&self) -> TaskRunDoc {
        let state = self.state.lock().unwrap();
        let logs_ref = if state.stdout.is_empty() { None } else {
            Some(TaskRunLogsRef { stdout: state.stdout.clone(), stderr: String::new() })
        };
        TaskRunDoc {
            kind: TASK_RUN_KIND.to_string(),
            name: self.name.clone(),
            version: "1.0.0".to_string(),
            metadata: DocumentMetadata {
                namespace: Some(self.namespace.clone()),
                owner: None, description: None, tags: vec![], extra: Default::default(),
            },
            spec: TaskRunSpec {
                task_run_request_ref: self.request.name.clone(),
                work_run_ref: String::new(),
                work_ref: self.request.spec.work_ref.clone(),
                task_ref: self.request.spec.task_ref.clone(),
                step_name: self.request.spec.step_name.clone(),
                attempt: 1, work_runner_ref: String::new(),
                task_runner_ref: self.task_runner_ref.clone(),
            },
            status: Some(TaskRunStatus {
                state: state.run_state.clone(),
                created_at: None, started_at: None, finished_at: None,
                outputs: self.build_outputs(&state.stdout),
                inputs: self.build_inputs(),
                state_history: vec![],
                logs_ref,
            }),
        }
    }

    fn start(&self) -> Result<(), TaskRunError> {
        {
            let mut g = self.state.lock().unwrap();
            if g.run_state != TaskRunState::Pending { return Err(TaskRunError::AlreadyStarted); }
            g.run_state = TaskRunState::Running;
        }
        let _ = self.sender.send(TaskRunEvent::StateChanged(TaskRunState::Running));
        let job_name = make_job_name(&self.cfg.job_name_prefix, &self.name);
        let cfg      = self.cfg.clone();
        let state    = Arc::clone(&self.state);
        let sender   = self.sender.clone();
        std::thread::spawn(move || run_kubernetes_job(cfg, job_name, state, sender));
        Ok(())
    }

    fn cancel(&self) -> Result<(), TaskRunError> {
        let job_to_delete = {
            let mut g = self.state.lock().unwrap();
            if matches!(g.run_state, TaskRunState::Succeeded | TaskRunState::Failed | TaskRunState::Cancelled) {
                return Err(TaskRunError::AlreadyFinished);
            }
            g.cancel_requested = true;
            g.run_state        = TaskRunState::Cancelled;
            if g.job_name.is_some() {
                g.deletion_initiated = true;
            }
            g.job_name.clone()
        };
        if let Some(job_name) = job_to_delete {
            spawn_cancel_deletion(self.cfg.namespace.clone(), job_name);
        }
        let _ = self.sender.send(TaskRunEvent::StateChanged(TaskRunState::Cancelled));
        let _ = self.sender.send(TaskRunEvent::Finished);
        Ok(())
    }

    fn subscribe(&self) -> TaskRunEventStream {
        Box::pin(CrossbeamStream::new(self.receiver.clone()))
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Derive a valid Kubernetes resource name from a prefix and task run name.
///
/// The result is at most 63 characters (Kubernetes limit) with no trailing hyphens.
fn make_job_name(prefix: &str, task_run_name: &str) -> String {
    let raw = format!("{}{}", prefix, task_run_name);
    let truncated = if raw.len() > 63 { &raw[..63] } else { &raw };
    truncated.trim_end_matches('-').to_string()
}

/// Spawn a detached thread that deletes the Kubernetes Job immediately.
///
/// Used by the cancel path to start cluster-side cleanup without waiting for
/// the exec thread's poll cycle to expire.
fn spawn_cancel_deletion(namespace: String, job_name: String) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt,
            Err(e) => {
                log_warn!("[k8s] cancel: failed to build runtime to delete job '{}': {}", job_name, e);
                return;
            }
        };
        rt.block_on(async {
            match Client::try_default().await {
                Ok(client) => delete_job(&client, &namespace, &job_name).await,
                Err(e)     => log_warn!("[k8s] cancel: kube client error deleting job '{}': {}", job_name, e),
            }
        });
    });
}
