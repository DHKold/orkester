//! Workspace client — fetches Namespace, Task, and Work definitions from the
//! Workspace server via hub messages.
//!
//! Instead of hitting an external HTTP server, the client sends
//! `http_request`-style messages directly to the Workspace server's hub
//! participant name (configurable, default `"workspace"`).  The Workspace
//! server replies with an `http_response` message containing the JSON body.
//!
//! This keeps everything in-process and avoids any network round-trip when
//! both servers run in the same Orkester instance.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::time::Duration;

use orkester_common::domain::{Namespace, Task, Work};
use orkester_common::messaging::Message;
use serde_json::{json, Value};
use tokio::sync::oneshot;

// ── WorkspaceClient ───────────────────────────────────────────────────────────

/// Async client that queries the Workspace server over the hub.
///
/// One instance is shared across the scheduler and worker tasks.
#[derive(Clone)]
pub struct WorkspaceClient {
    inner: Arc<WorkspaceClientInner>,
}

struct WorkspaceClientInner {
    /// Hub participant name of the Workspace server (default: `"workspace"`).
    target: String,
    /// Channel to send messages into the hub.
    to_hub: std::sync::mpsc::Sender<Message>,
    /// Pending requests waiting for a response, keyed by correlation id.
    pending: Mutex<HashMap<u64, oneshot::Sender<Value>>>,
    next_id: AtomicU64,
}

impl WorkspaceClient {
    pub fn new(target: impl Into<String>, to_hub: std::sync::mpsc::Sender<Message>) -> Self {
        Self {
            inner: Arc::new(WorkspaceClientInner {
                target: target.into(),
                to_hub,
                pending: Mutex::new(HashMap::new()),
                next_id: AtomicU64::new(1),
            }),
        }
    }

    /// Must be called from the server's event loop whenever an
    /// `http_response` message addressed to this client arrives from the hub.
    pub fn handle_response(&self, msg: Message) {
        let corr_id = msg
            .content
            .get("correlation_id")
            .and_then(|v| v.as_u64());
        if let Some(id) = corr_id {
            if let Some(tx) = self.inner.pending.lock().unwrap().remove(&id) {
                let body = msg.content.get("body").cloned().unwrap_or(Value::Null);
                let _ = tx.send(body);
            }
        }
    }

    // ── Namespace queries ─────────────────────────────────────────────────

    pub async fn get_namespace(&self, name: &str) -> ClientResult<Namespace> {
        let body = self
            .get(&format!("/v1/namespaces/{name}"))
            .await?;
        parse_one(body)
    }

    pub async fn list_namespaces(&self) -> ClientResult<Vec<Namespace>> {
        let body = self.get("/v1/namespaces").await?;
        parse_list(body, "namespaces")
    }

    // ── Task queries ──────────────────────────────────────────────────────

    pub async fn get_task(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> ClientResult<Task> {
        let body = self
            .get(&format!("/v1/namespaces/{namespace}/tasks/{name}/{version}"))
            .await?;
        parse_one(body)
    }

    pub async fn list_tasks(&self, namespace: &str) -> ClientResult<Vec<Task>> {
        let body = self
            .get(&format!("/v1/namespaces/{namespace}/tasks"))
            .await?;
        parse_list(body, "tasks")
    }

    // ── Work queries ──────────────────────────────────────────────────────

    pub async fn get_work(
        &self,
        namespace: &str,
        name: &str,
        version: &str,
    ) -> ClientResult<Work> {
        let body = self
            .get(&format!("/v1/namespaces/{namespace}/works/{name}/{version}"))
            .await?;
        parse_one(body)
    }

    pub async fn list_works(&self, namespace: &str) -> ClientResult<Vec<Work>> {
        let body = self
            .get(&format!("/v1/namespaces/{namespace}/works"))
            .await?;
        parse_list(body, "works")
    }

    // ── Internal ──────────────────────────────────────────────────────────

    async fn get(&self, path: &str) -> ClientResult<Value> {
        let corr_id = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel::<Value>();
        self.inner.pending.lock().unwrap().insert(corr_id, tx);

        let msg = Message::new(
            corr_id,
            "",
            self.inner.target.as_str(),
            "http_request",
            json!({
                "correlation_id": corr_id,
                "method": "GET",
                "path": path,
            }),
        );

        if self.inner.to_hub.send(msg).is_err() {
            self.inner.pending.lock().unwrap().remove(&corr_id);
            return Err(ClientError::HubDisconnected);
        }

        match tokio::time::timeout(Duration::from_secs(10), rx).await {
            Ok(Ok(body)) => Ok(body),
            Ok(Err(_)) => Err(ClientError::HubDisconnected),
            Err(_) => {
                self.inner.pending.lock().unwrap().remove(&corr_id);
                Err(ClientError::Timeout)
            }
        }
    }
}

// ── Error type ────────────────────────────────────────────────────────────────

pub type ClientResult<T> = Result<T, ClientError>;

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("workspace server did not respond in time")]
    Timeout,
    #[error("hub channel disconnected")]
    HubDisconnected,
    #[error("workspace returned status {status}: {message}")]
    Api { status: u16, message: String },
    #[error("deserialization error: {0}")]
    Deserialize(String),
}

// ── Parsing helpers ───────────────────────────────────────────────────────────

fn parse_one<T: serde::de::DeserializeOwned>(body: Value) -> ClientResult<T> {
    serde_json::from_value(body).map_err(|e| ClientError::Deserialize(e.to_string()))
}

fn parse_list<T: serde::de::DeserializeOwned>(body: Value, key: &str) -> ClientResult<Vec<T>> {
    let arr = body
        .get(key)
        .cloned()
        .unwrap_or(Value::Array(vec![]));
    serde_json::from_value(arr).map_err(|e| ClientError::Deserialize(e.to_string()))
}
