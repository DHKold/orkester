//! HTTP task runner — fires an HTTP POST and polls for job completion.
//!
//! The runner POSTs a JSON payload derived from the resolved task inputs to
//! the configured URL.  The endpoint is expected to return a JSON body with
//! an `"id"` field; the runner then polls `{url}/{id}` until it receives a
//! terminal `"status"` (`"succeeded"` | `"failed"` | `"cancelled"`).
//!
//! # Runner config keys (inside `spec.config`)
//!
//! | Key             | Type   | Description                                        |
//! |-----------------|--------|----------------------------------------------------|
//! | `url`           | string | Base URL for the task endpoint (required).        |
//! | `timeout_secs`  | u64    | Hard deadline for the entire run (default: 3600). |
//! | `poll_secs`     | u64    | Polling interval in seconds (default: 5).         |

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

// ─── HttpTaskRunner ────────────────────────────────────────────────────────────

/// Executes tasks by sending an HTTP POST request and polling for completion.
#[derive(Debug)]
pub struct HttpTaskRunner {
    name:      String,
    namespace: String,
    spec:      TaskRunnerSpec,
    state:     Mutex<TaskRunnerState>,
}

impl HttpTaskRunner {
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

impl TaskRunner for HttpTaskRunner {
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
        let url = request
            .spec
            .execution
            .config
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TaskRunnerError::Other("missing 'url' in http runner config".into()))?
            .to_string();
        let timeout_secs = request
            .spec.execution.config.get("timeout_secs")
            .and_then(|v| v.as_u64()).unwrap_or(3600);
        let poll_secs = request
            .spec.execution.config.get("poll_secs")
            .and_then(|v| v.as_u64()).unwrap_or(5);

        let run = HttpTaskRun::new(Uuid::new_v4().to_string(), self.namespace.clone(),
            self.self_ref(), request, url, timeout_secs, poll_secs);
        Ok(Box::new(run))
    }
}

// ─── HttpTaskRun ──────────────────────────────────────────────────────────────

#[derive(Debug)]
struct HttpTaskRun {
    name:             String,
    namespace:        String,
    task_runner_ref:  String,
    request:          TaskRunRequestDoc,
    url:              String,
    timeout_secs:     u64,
    poll_secs:        u64,
    state:            Arc<Mutex<HttpTaskRunState>>,
    sender:           crossbeam_channel::Sender<TaskRunEvent>,
    receiver:         crossbeam_channel::Receiver<TaskRunEvent>,
}

#[derive(Debug, Default)]
struct HttpTaskRunState {
    run_state:        TaskRunState,
    cancel_requested: bool,
}

impl HttpTaskRun {
    fn new(
        name:            String,
        namespace:       String,
        task_runner_ref: String,
        request:         TaskRunRequestDoc,
        url:             String,
        timeout_secs:    u64,
        poll_secs:       u64,
    ) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            name, namespace, task_runner_ref, request,
            url, timeout_secs, poll_secs,
            state: Arc::new(Mutex::new(HttpTaskRunState {
                run_state: TaskRunState::Pending, cancel_requested: false,
            })),
            sender: tx, receiver: rx,
        }
    }
}

impl TaskRun for HttpTaskRun {
    fn as_doc(&self) -> TaskRunDoc {
        let state = self.state.lock().unwrap();
        TaskRunDoc {
            kind:    TASK_RUN_KIND.to_string(),
            name:    self.name.clone(),
            version: "1.0.0".to_string(),
            metadata: DocumentMetadata {
                namespace: Some(self.namespace.clone()),
                owner: None, description: None, tags: vec![], extra: Default::default(),
            },
            spec: TaskRunSpec {
                task_run_request_ref: self.request.name.clone(),
                work_run_ref:         String::new(),
                work_ref:             self.request.spec.work_ref.clone(),
                task_ref:             self.request.spec.task_ref.clone(),
                step_name:            self.request.spec.step_name.clone(),
                attempt:              1,
                work_runner_ref:      String::new(),
                task_runner_ref:      self.task_runner_ref.clone(),
            },
            status: Some(TaskRunStatus {
                state:         state.run_state.clone(),
                created_at:    None, started_at: None, finished_at: None,
                outputs:       Default::default(),
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

        let payload = build_input_payload(&self.request);
        let shared   = Arc::clone(&self.state);
        let sender   = self.sender.clone();
        let url      = self.url.clone();
        let timeout  = Duration::from_secs(self.timeout_secs);
        let poll     = Duration::from_secs(self.poll_secs);

        std::thread::spawn(move || {
            run_http_task(url, payload, timeout, poll, shared, sender);
        });
        Ok(())
    }

    fn cancel(&self) -> Result<(), TaskRunError> {
        let mut g = self.state.lock().unwrap();
        if matches!(g.run_state, TaskRunState::Succeeded | TaskRunState::Failed | TaskRunState::Cancelled) {
            return Err(TaskRunError::AlreadyFinished);
        }
        g.cancel_requested = true;
        g.run_state = TaskRunState::Cancelled;
        drop(g);
        let _ = self.sender.send(TaskRunEvent::StateChanged(TaskRunState::Cancelled));
        let _ = self.sender.send(TaskRunEvent::Finished);
        Ok(())
    }

    fn subscribe(&self) -> TaskRunEventStream {
        Box::pin(CrossbeamStream::new(self.receiver.clone()))
    }
}

// ─── HTTP helpers ─────────────────────────────────────────────────────────────

/// Build a JSON map from the resolved task inputs.
fn build_input_payload(request: &TaskRunRequestDoc) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for input in &request.spec.inputs {
        let val = match &input.from {
            workaholic::TaskInputSource::Literal { value } => value.clone(),
            workaholic::TaskInputSource::ArtifactRef { uri } => {
                serde_json::Value::String(uri.clone())
            }
        };
        map.insert(input.name.clone(), val);
    }
    serde_json::Value::Object(map)
}

/// Synchronous HTTP execution loop running on a background thread.
fn run_http_task(
    url:     String,
    payload: serde_json::Value,
    timeout: Duration,
    poll:    Duration,
    state:   Arc<Mutex<HttpTaskRunState>>,
    sender:  crossbeam_channel::Sender<TaskRunEvent>,
) {
    if state.lock().unwrap().cancel_requested {
        let _ = sender.send(TaskRunEvent::Finished);
        return;
    }
    let job_id = match http_post_json(&url, &payload) {
        Ok(id) => id,
        Err(e) => {
            log::error!("[http runner] POST failed: {}", e);
            state.lock().unwrap().run_state = TaskRunState::Failed;
            let _ = sender.send(TaskRunEvent::StateChanged(TaskRunState::Failed));
            let _ = sender.send(TaskRunEvent::Finished);
            return;
        }
    };

    let deadline = std::time::Instant::now() + timeout;
    let poll_url = format!("{}/{}", url.trim_end_matches('/'), job_id);

    loop {
        if state.lock().unwrap().cancel_requested {
            let _ = sender.send(TaskRunEvent::Finished);
            return;
        }
        if std::time::Instant::now() >= deadline {
            log::warn!("[http runner] job '{}' timed out", job_id);
            state.lock().unwrap().run_state = TaskRunState::Failed;
            let _ = sender.send(TaskRunEvent::StateChanged(TaskRunState::Failed));
            let _ = sender.send(TaskRunEvent::Finished);
            return;
        }

        match http_get_status(&poll_url) {
            Ok(status) => {
                let final_state = map_http_status(&status);
                if let Some(fs) = final_state {
                    state.lock().unwrap().run_state = fs.clone();
                    let _ = sender.send(TaskRunEvent::StateChanged(fs));
                    let _ = sender.send(TaskRunEvent::Finished);
                    return;
                }
            }
            Err(e) => {
                log::warn!("[http runner] poll error for job '{}': {}", job_id, e);
            }
        }

        std::thread::sleep(poll);
    }
}

/// POST JSON using only the standard library (no external HTTP client).
fn http_post_json(url: &str, payload: &serde_json::Value) -> Result<String, String> {
    let body = serde_json::to_string(payload).map_err(|e| e.to_string())?;
    let (host, path) = split_url(url)?;
    let request = format!(
        "POST {} HTTP/1.0\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        path, host, body.len(), body
    );
    let response = tcp_exchange(&host, &request)?;
    let json_body = extract_http_body(&response)?;
    let v: serde_json::Value = serde_json::from_str(&json_body).map_err(|e| e.to_string())?;
    v.get("id")
        .and_then(|id| id.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "response missing 'id' field".into())
}

/// GET the job status JSON.
fn http_get_status(url: &str) -> Result<String, String> {
    let (host, path) = split_url(url)?;
    let request = format!(
        "GET {} HTTP/1.0\r\nHost: {}\r\nConnection: close\r\n\r\n",
        path, host
    );
    let response = tcp_exchange(&host, &request)?;
    let json_body = extract_http_body(&response)?;
    let v: serde_json::Value = serde_json::from_str(&json_body).map_err(|e| e.to_string())?;
    Ok(v.get("status").and_then(|s| s.as_str()).unwrap_or("").to_string())
}

/// Map HTTP status string → terminal TaskRunState (None = still running).
fn map_http_status(status: &str) -> Option<TaskRunState> {
    match status {
        "succeeded" => Some(TaskRunState::Succeeded),
        "failed"    => Some(TaskRunState::Failed),
        "cancelled" => Some(TaskRunState::Cancelled),
        _           => None,
    }
}

/// Parse `http://host:port/path` into `("host:port", "/path")`.
fn split_url(url: &str) -> Result<(String, String), String> {
    let stripped = url.strip_prefix("http://")
        .ok_or_else(|| format!("only http:// URLs are supported, got: {}", url))?;
    let slash = stripped.find('/').unwrap_or(stripped.len());
    let host = stripped[..slash].to_string();
    let path = if slash < stripped.len() { stripped[slash..].to_string() } else { "/".to_string() };
    Ok((host, path))
}

/// Open a TCP connection, write `request`, read full response.
fn tcp_exchange(host: &str, request: &str) -> Result<String, String> {
    use std::io::{Read, Write};
    let addr = if host.contains(':') { host.to_string() } else { format!("{}:80", host) };
    let mut stream = std::net::TcpStream::connect(&addr)
        .map_err(|e| format!("connect to '{}': {}", addr, e))?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| e.to_string())?;
    stream.write_all(request.as_bytes()).map_err(|e| e.to_string())?;
    let mut buf = String::new();
    stream.read_to_string(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

/// Strip HTTP response headers; return only the body.
fn extract_http_body(response: &str) -> Result<String, String> {
    response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .ok_or_else(|| "malformed HTTP response (no header/body separator)".into())
}

