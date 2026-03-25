//! Workspace client — fetches Namespace, Task, and Work definitions from the
//! Workspace server via direct hub messages.
//!
//! Uses the `workspace_request` / `workspace_response` message protocol so
//! requests bypass the REST layer entirely and go straight to the Workspace
//! server's in-process store.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use orkester_common::domain::{Namespace, Task, Work};
use orkester_common::messaging::Message;
use serde_json::{json, Value};
use tokio::sync::oneshot;

// ── WorkspaceClient ───────────────────────────────────────────────────────────

/// Sends `workspace_request` messages directly to the Workspace server and
/// awaits `workspace_response` replies through the shared hub.
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

    /// Must be called from the server's event loop whenever a
    /// `workspace_response` message arrives from the hub.
    pub fn handle_response(&self, msg: Message) {
        let corr_id = msg.content.get("correlation_id").and_then(|v| v.as_u64());
        if let Some(id) = corr_id {
            if let Some(tx) = self.inner.pending.lock().unwrap().remove(&id) {
                let _ = tx.send(msg.content);
            }
        }
    }

    // ── Namespace queries ─────────────────────────────────────────────────

    pub async fn get_namespace(&self, name: &str) -> ClientResult<Namespace> {
        let resp = self.request(json!({ "op": "get_namespace", "name": name })).await?;
        parse_object(resp)
    }

    pub async fn list_namespaces(&self) -> ClientResult<Vec<Namespace>> {
        let resp = self.request(json!({ "op": "list_namespaces" })).await?;
        parse_objects(resp)
    }

    // ── Task queries ──────────────────────────────────────────────────────

    pub async fn get_task(&self, namespace: &str, name: &str, version: &str) -> ClientResult<Task> {
        let resp = self
            .request(json!({ "op": "get_task", "namespace": namespace, "name": name, "version": version }))
            .await?;
        parse_object(resp)
    }

    pub async fn list_tasks(&self, namespace: &str) -> ClientResult<Vec<Task>> {
        let resp = self.request(json!({ "op": "list_tasks", "namespace": namespace })).await?;
        parse_objects(resp)
    }

    // ── Work queries ──────────────────────────────────────────────────────

    pub async fn get_work(&self, namespace: &str, name: &str, version: &str) -> ClientResult<Work> {
        let resp = self
            .request(json!({ "op": "get_work", "namespace": namespace, "name": name, "version": version }))
            .await?;
        parse_object(resp)
    }

    pub async fn list_works(&self, namespace: &str) -> ClientResult<Vec<Work>> {
        let resp = self.request(json!({ "op": "list_works", "namespace": namespace })).await?;
        parse_objects(resp)
    }

    // ── Internal ──────────────────────────────────────────────────────────

    async fn request(&self, mut payload: Value) -> ClientResult<Value> {
        let corr_id = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
        payload["correlation_id"] = json!(corr_id);

        let (tx, rx) = oneshot::channel::<Value>();
        self.inner.pending.lock().unwrap().insert(corr_id, tx);

        let msg = Message::new(
            corr_id,
            "",
            self.inner.target.as_str(),
            "workspace_request",
            payload,
        );

        if self.inner.to_hub.send(msg).is_err() {
            self.inner.pending.lock().unwrap().remove(&corr_id);
            return Err(ClientError::HubDisconnected);
        }

        match tokio::time::timeout(Duration::from_secs(10), rx).await {
            Ok(Ok(resp)) => Ok(resp),
            Ok(Err(_))   => Err(ClientError::HubDisconnected),
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
    #[error("workspace returned error: {0}")]
    NotFound(String),
    #[error("deserialization error: {0}")]
    Deserialize(String),
}

// ── Response parsing ──────────────────────────────────────────────────────────

fn check_ok(resp: &Value) -> ClientResult<()> {
    if resp.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        Ok(())
    } else {
        let msg = resp.get("error").and_then(|v| v.as_str()).unwrap_or("unknown error");
        Err(ClientError::NotFound(msg.to_string()))
    }
}

fn parse_object<T: serde::de::DeserializeOwned>(resp: Value) -> ClientResult<T> {
    check_ok(&resp)?;
    let obj = resp.get("object").cloned().unwrap_or(Value::Null);
    serde_json::from_value(obj).map_err(|e| ClientError::Deserialize(e.to_string()))
}

fn parse_objects<T: serde::de::DeserializeOwned>(resp: Value) -> ClientResult<Vec<T>> {
    check_ok(&resp)?;
    let arr = resp.get("objects").cloned().unwrap_or(Value::Array(vec![]));
    serde_json::from_value(arr).map_err(|e| ClientError::Deserialize(e.to_string()))
}
