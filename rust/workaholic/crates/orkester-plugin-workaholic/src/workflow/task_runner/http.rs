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
//! | `timeout_secs`       | u64    | Hard deadline for the entire run (default: 3600).       |
//! | `poll_secs`          | u64    | Polling interval in seconds (default: 5).               |
//! | `max_retries`        | u64    | Retry count on 429/5xx or network errors (default: 3).  |
//! | `retry_delay_secs`   | u64    | Delay between retries in seconds (default: 2).          |
//! | `auth_type`          | string | `bearer` or `basic` (optional).                         |
//! | `auth_token`         | string | Bearer token (required for `auth_type = bearer`).       |
//! | `auth_user`          | string | Username (required for `auth_type = basic`).            |
//! | `auth_password`      | string | Password (required for `auth_type = basic`).            |

use std::sync::{Arc, Mutex};
use std::time::Duration;

use uuid::Uuid;
use workaholic::{
    DocumentMetadata, TaskRunDoc, TaskRunRequestDoc, TaskRunSpec, TaskRunState, TaskRunStatus,
    TaskRunnerDoc, TaskRunnerSpec, TaskRunnerState, TaskRunnerStatus, TASK_RUN_KIND,
    TASK_RUNNER_KIND,
};
use orkester_plugin::{log_error, log_warn};

use super::traits::{TaskRun, TaskRunError, TaskRunEvent, TaskRunEventStream, TaskRunner, TaskRunnerError};
use super::stream_adapter::CrossbeamStream;

// ─── HttpRunConfig ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct HttpRunConfig {
    url:              String,
    timeout_secs:     u64,
    poll_secs:        u64,
    max_retries:      u32,
    retry_delay_secs: u64,
    auth_type:        Option<String>,
    auth_token:       Option<String>,
    auth_user:        Option<String>,
    auth_password:    Option<String>,
}

impl HttpRunConfig {
    fn from_config(cfg: &serde_json::Value) -> Result<Self, String> {
        let url = cfg.get("url").and_then(|v| v.as_str())
            .ok_or("missing 'url' in http runner config")?
            .to_string();
        Ok(Self {
            url,
            timeout_secs:     cfg.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(3600),
            poll_secs:        cfg.get("poll_secs").and_then(|v| v.as_u64()).unwrap_or(5),
            max_retries:      cfg.get("max_retries").and_then(|v| v.as_u64()).unwrap_or(3) as u32,
            retry_delay_secs: cfg.get("retry_delay_secs").and_then(|v| v.as_u64()).unwrap_or(2),
            auth_type:        cfg.get("auth_type").and_then(|v| v.as_str()).map(String::from),
            auth_token:       cfg.get("auth_token").and_then(|v| v.as_str()).map(String::from),
            auth_user:        cfg.get("auth_user").and_then(|v| v.as_str()).map(String::from),
            auth_password:    cfg.get("auth_password").and_then(|v| v.as_str()).map(String::from),
        })
    }
}

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
        let cfg = HttpRunConfig::from_config(&request.spec.execution.config)
            .map_err(TaskRunnerError::Other)?;
        let run = HttpTaskRun::new(Uuid::new_v4().to_string(), self.namespace.clone(),
            self.self_ref(), request, cfg);
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
    cfg:              HttpRunConfig,
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
        cfg:             HttpRunConfig,
    ) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            name, namespace, task_runner_ref, request, cfg,
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
                inputs:        Default::default(),
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

        let payload  = build_input_payload(&self.request);
        let shared   = Arc::clone(&self.state);
        let sender   = self.sender.clone();
        let cfg      = self.cfg.clone();

        std::thread::spawn(move || {
            run_http_task(cfg, payload, shared, sender);
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

/// Entry point for the background HTTP task thread.
fn run_http_task(
    cfg:     HttpRunConfig,
    payload: serde_json::Value,
    state:   Arc<Mutex<HttpTaskRunState>>,
    sender:  crossbeam_channel::Sender<TaskRunEvent>,
) {
    if state.lock().unwrap().cancel_requested {
        let _ = sender.send(TaskRunEvent::Finished);
        return;
    }
    let retry    = Duration::from_secs(cfg.retry_delay_secs);
    let auth_hdr = build_auth_header(&cfg);
    let job_id = match with_retry(cfg.max_retries, retry, || {
        http_post_json(&cfg.url, &payload, auth_hdr.as_deref())
    }) {
        Ok(id) => id,
        Err(e) => {
            log_error!("[http runner] POST failed: {}", e);
            finish_run(&state, &sender, TaskRunState::Failed);
            return;
        }
    };
    let poll_url = format!("{}/{}", cfg.url.trim_end_matches('/'), job_id);
    poll_for_completion(cfg, auth_hdr, poll_url, state, sender);
}

/// Poll `poll_url` until a terminal status is received or the deadline is reached.
fn poll_for_completion(
    cfg:      HttpRunConfig,
    auth_hdr: Option<String>,
    poll_url: String,
    state:    Arc<Mutex<HttpTaskRunState>>,
    sender:   crossbeam_channel::Sender<TaskRunEvent>,
) {
    let deadline = std::time::Instant::now() + Duration::from_secs(cfg.timeout_secs);
    let poll     = Duration::from_secs(cfg.poll_secs);
    let retry    = Duration::from_secs(cfg.retry_delay_secs);
    loop {
        if state.lock().unwrap().cancel_requested {
            let _ = sender.send(TaskRunEvent::Finished);
            return;
        }
        if std::time::Instant::now() >= deadline {
            log_warn!("[http runner] timed out polling '{}'", poll_url);
            finish_run(&state, &sender, TaskRunState::Failed);
            return;
        }
        match with_retry(cfg.max_retries, retry, || http_get_status(&poll_url, auth_hdr.as_deref())) {
            Ok(status) => {
                if let Some(fs) = map_http_status(&status) {
                    finish_run(&state, &sender, fs);
                    return;
                }
            }
            Err(e) => log_warn!("[http runner] poll error at '{}': {}", poll_url, e),
        }
        std::thread::sleep(poll);
    }
}

/// Transition run to a terminal state and notify the event stream.
fn finish_run(
    state:  &Mutex<HttpTaskRunState>,
    sender: &crossbeam_channel::Sender<TaskRunEvent>,
    fs:     TaskRunState,
) {
    state.lock().unwrap().run_state = fs.clone();
    let _ = sender.send(TaskRunEvent::StateChanged(fs));
    let _ = sender.send(TaskRunEvent::Finished);
}

/// Retry `f` up to `max_retries` additional times on transient errors.
fn with_retry<T>(
    max_retries: u32,
    delay:       Duration,
    mut f:       impl FnMut() -> Result<T, String>,
) -> Result<T, String> {
    let mut last_err = String::new();
    for attempt in 0..=max_retries {
        match f() {
            Ok(v)  => return Ok(v),
            Err(e) => { last_err = e; if attempt < max_retries { std::thread::sleep(delay); } }
        }
    }
    Err(last_err)
}

/// POST JSON and return the remote job `id` from the response.
fn http_post_json(url: &str, payload: &serde_json::Value, auth: Option<&str>) -> Result<String, String> {
    let body = serde_json::to_string(payload).map_err(|e| e.to_string())?;
    let mut req = ureq::post(url).set("Content-Type", "application/json");
    if let Some(a) = auth { req = req.set("Authorization", a); }
    let resp = match req.send_string(&body) {
        Ok(r)                                                   => r,
        Err(ureq::Error::Status(c, _)) if c == 429 || c >= 500 => return Err(format!("HTTP {}", c)),
        Err(e)                                                  => return Err(e.to_string()),
    };
    let json: serde_json::Value = serde_json::from_reader(resp.into_reader())
        .map_err(|e| e.to_string())?;
    json.get("id").and_then(|v| v.as_str()).map(String::from)
        .ok_or_else(|| "response missing 'id' field".into())
}

/// GET the remote job status string.
fn http_get_status(url: &str, auth: Option<&str>) -> Result<String, String> {
    let mut req = ureq::get(url);
    if let Some(a) = auth { req = req.set("Authorization", a); }
    let resp = match req.call() {
        Ok(r)                                                   => r,
        Err(ureq::Error::Status(c, _)) if c == 429 || c >= 500 => return Err(format!("HTTP {}", c)),
        Err(e)                                                  => return Err(e.to_string()),
    };
    let json: serde_json::Value = serde_json::from_reader(resp.into_reader())
        .map_err(|e| e.to_string())?;
    Ok(json.get("status").and_then(|s| s.as_str()).unwrap_or("").to_string())
}

/// Map HTTP status string to a terminal `TaskRunState` (returns `None` while still running).
fn map_http_status(status: &str) -> Option<TaskRunState> {
    match status {
        "succeeded" => Some(TaskRunState::Succeeded),
        "failed"    => Some(TaskRunState::Failed),
        "cancelled" => Some(TaskRunState::Cancelled),
        _           => None,
    }
}

/// Compute the `Authorization` header value from run config.
fn build_auth_header(cfg: &HttpRunConfig) -> Option<String> {
    match cfg.auth_type.as_deref() {
        Some("bearer") => cfg.auth_token.as_deref().map(|t| format!("Bearer {}", t)),
        Some("basic")  => {
            let user = cfg.auth_user.as_deref().unwrap_or("");
            let pass = cfg.auth_password.as_deref().unwrap_or("");
            Some(format!("Basic {}", base64_encode(format!("{}:{}", user, pass).as_bytes())))
        }
        _              => None,
    }
}

/// Minimal RFC 4648 base64 encoder (used for Basic auth headers).
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        out.push(CHARS[((b0 >> 2) & 0x3F) as usize] as char);
        out.push(CHARS[(((b0 << 4) | (b1 >> 4)) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 { CHARS[(((b1 << 2) | (b2 >> 6)) & 0x3F) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { CHARS[(b2 & 0x3F) as usize] as char } else { '=' });
    }
    out
}

