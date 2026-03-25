//! API handler — processes `http_request` messages for the Workflows server.
//!
//! # Routes
//!
//! | Method | Path                                                   | Description                        |
//! |--------|--------------------------------------------------------|------------------------------------|
//! | GET    | /v1/namespaces/{ns}/workflows                          | List all workflows in a namespace  |
//! | POST   | /v1/namespaces/{ns}/workflows                          | Create a new workflow              |
//! | GET    | /v1/namespaces/{ns}/workflows/{id}                     | Get a specific workflow            |
//! | PUT    | /v1/namespaces/{ns}/workflows/{id}                     | Update a workflow (e.g. pause)     |
//! | DELETE | /v1/namespaces/{ns}/workflows/{id}                     | Cancel + delete a workflow         |
//! | GET    | /v1/namespaces/{ns}/crons                              | List all crons in a namespace      |
//! | POST   | /v1/namespaces/{ns}/crons                              | Create a new cron                  |
//! | GET    | /v1/namespaces/{ns}/crons/{id}                         | Get a specific cron                |
//! | PUT    | /v1/namespaces/{ns}/crons/{id}                         | Update a cron                      |
//! | DELETE | /v1/namespaces/{ns}/crons/{id}                         | Delete a cron                      |

use std::sync::mpsc;

use chrono::Utc;
use orkester_common::messaging::Message;
use orkester_common::{log_debug, log_warn};
use serde_json::{json, Value};

use super::model::{Cron, Workflow, WorkflowStatus};
use super::store::WorkflowsStore;

/// Routes that must be registered with REST servers at startup.
pub const ROUTES: &[(&str, &str)] = &[
    ("GET", "/v1/namespaces/{ns}/workflows"),
    ("POST", "/v1/namespaces/{ns}/workflows"),
    ("GET", "/v1/namespaces/{ns}/workflows/{id}"),
    ("PUT", "/v1/namespaces/{ns}/workflows/{id}"),
    ("DELETE", "/v1/namespaces/{ns}/workflows/{id}"),
    ("GET", "/v1/namespaces/{ns}/workflows/{id}/steps"),
    ("GET", "/v1/namespaces/{ns}/workflows/{id}/steps/{step_id}"),
    ("GET", "/v1/namespaces/{ns}/workflows/{id}/steps/{step_id}/logs"),
    ("GET", "/v1/namespaces/{ns}/crons"),
    ("POST", "/v1/namespaces/{ns}/crons"),
    ("GET", "/v1/namespaces/{ns}/crons/{id}"),
    ("PUT", "/v1/namespaces/{ns}/crons/{id}"),
    ("DELETE", "/v1/namespaces/{ns}/crons/{id}"),
];

// ── ApiHandler ────────────────────────────────────────────────────────────────

pub struct ApiHandler {
    pub store: WorkflowsStore,
    pub to_hub: mpsc::Sender<Message>,
    pub spawn_tx: tokio::sync::mpsc::UnboundedSender<Workflow>,
    pub metrics_target: String,
}

impl ApiHandler {
    pub async fn handle(&self, msg: Message) {
        let method = msg
            .content
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET")
            .to_uppercase();
        let path = msg
            .content
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let body = msg.content.get("body").cloned().unwrap_or(Value::Null);
        let corr_id = msg
            .content
            .get("correlation_id")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let source = msg.source.clone();

        log_debug!("{} {} (correlation_id={})", method, path, corr_id);

        let (status, resp_body) = self.dispatch(&method, &path, body).await;
        self.reply(&source, corr_id, status, resp_body);
    }

    async fn dispatch(&self, method: &str, path: &str, body: Value) -> (u16, Value) {
        let segs: Vec<&str> = path.trim_start_matches('/').split('/').collect();

        match (method, segs.as_slice()) {
            // ── Workflows ─────────────────────────────────────────────────
            ("GET", ["v1", "namespaces", ns, "workflows"]) => {
                match self.store.list_workflows(ns).await {
                    Ok(list) => (200, json!({ "workflows": list })),
                    Err(e) => server_error(e),
                }
            }

            ("POST", ["v1", "namespaces", ns, "workflows"]) => {
                match serde_json::from_value::<Workflow>(body) {
                    Err(e) => (400, json!({ "error": format!("invalid body: {e}") })),
                    Ok(mut wf) => {
                        // Ensure the namespace matches the URL, stamp timestamps.
                        wf.namespace = ns.to_string();
                        if wf.id.is_empty() {
                            wf.id = uuid::Uuid::new_v4().to_string();
                        }
                        let now = Utc::now();
                        wf.created_at = now;
                        wf.updated_at = now;
                        wf.status = WorkflowStatus::Waiting;

                        match self.store.put_workflow(&wf).await {
                            Ok(()) => {
                                let _ = self.spawn_tx.send(wf.clone());
                                self.send_metric("workflows.created_total", "increment", 1.0);
                                (201, json!(wf))
                            }
                            Err(e) => server_error(e),
                        }
                    }
                }
            }

            ("GET", ["v1", "namespaces", ns, "workflows", id]) => {
                match self.store.get_workflow(ns, id).await {
                    Ok(wf) => (200, json!(wf)),
                    Err(e) => not_found_or_error(e),
                }
            }

            ("PUT", ["v1", "namespaces", ns, "workflows", id]) => {
                // Fetch the existing record first, then merge the supplied fields.
                match self.store.get_workflow(ns, id).await {
                    Err(e) => not_found_or_error(e),
                    Ok(mut wf) => {
                        // Allow updating: status (e.g. pause/resume), work_context,
                        // schedule, execution policy.
                        if let Some(status) = body.get("status") {
                            match serde_json::from_value::<WorkflowStatus>(status.clone()) {
                                Ok(s) => wf.status = s,
                                Err(e) => {
                                    return (
                                        400,
                                        json!({ "error": format!("invalid status: {e}") }),
                                    )
                                }
                            }
                        }
                        if let Some(ctx) = body.get("work_context").and_then(|v| v.as_object()) {
                            for (k, v) in ctx {
                                wf.work_context.insert(k.clone(), v.clone());
                            }
                        }
                        if let Some(sched) = body.get("schedule") {
                            match serde_json::from_value(sched.clone()) {
                                Ok(s) => wf.schedule = s,
                                Err(e) => {
                                    return (
                                        400,
                                        json!({ "error": format!("invalid schedule: {e}") }),
                                    )
                                }
                            }
                        }
                        wf.updated_at = Utc::now();
                        match self.store.put_workflow(&wf).await {
                            Ok(()) => (200, json!(wf)),
                            Err(e) => server_error(e),
                        }
                    }
                }
            }

            ("DELETE", ["v1", "namespaces", ns, "workflows", id]) => {
                match self.store.delete_workflow(ns, id).await {
                    Ok(()) => (204, Value::Null),
                    Err(e) => not_found_or_error(e),
                }
            }
            // ── Workflow steps ────────────────────────────────────────────────────────

            ("GET", ["v1", "namespaces", ns, "workflows", id, "steps"]) => {
                match self.store.get_workflow(ns, id).await {
                    Ok(wf) => (200, json!({ "steps": wf.steps })),
                    Err(e) => not_found_or_error(e),
                }
            }

            ("GET", ["v1", "namespaces", ns, "workflows", id, "steps", step_id]) => {
                match self.store.get_workflow(ns, id).await {
                    Ok(wf) => match wf.steps.get(*step_id) {
                        Some(state) => (200, json!(state)),
                        None => (404, json!({ "error": "step not found" })),
                    },
                    Err(e) => not_found_or_error(e),
                }
            }

            ("GET", ["v1", "namespaces", ns, "workflows", id, "steps", step_id, "logs"]) => {
                match self.store.get_workflow(ns, id).await {
                    Ok(wf) => match wf.steps.get(*step_id) {
                        Some(state) => (200, json!({ "logs": state.logs })),
                        None => (404, json!({ "error": "step not found" })),
                    },
                    Err(e) => not_found_or_error(e),
                }
            }
            // ── Crons ─────────────────────────────────────────────────────
            ("GET", ["v1", "namespaces", ns, "crons"]) => match self.store.list_crons(ns).await {
                Ok(list) => (200, json!({ "crons": list })),
                Err(e) => server_error(e),
            },

            ("POST", ["v1", "namespaces", ns, "crons"]) => {
                match serde_json::from_value::<Cron>(body) {
                    Err(e) => (400, json!({ "error": format!("invalid body: {e}") })),
                    Ok(mut cron) => {
                        cron.namespace = ns.to_string();
                        if cron.id.is_empty() {
                            cron.id = uuid::Uuid::new_v4().to_string();
                        }
                        let now = Utc::now();
                        cron.created_at = now;
                        cron.updated_at = now;
                        // Pre-compute the first fire time so the scheduler picks it up immediately.
                        if cron.next_fire_at.is_none() {
                            cron.next_fire_at = Cron::next_fire_after(&cron.schedule, now);
                        }

                        match self.store.put_cron(&cron).await {
                            Err(e) => return server_error(e),
                            Ok(()) => {}
                        }
                        // Update global index so the scheduler can find it.
                        if let Err(e) = self.store.index_cron(&cron).await {
                            log_warn!("Failed to index cron '{}': {}", cron.id, e);
                        }
                        (201, json!(cron))
                    }
                }
            }

            ("GET", ["v1", "namespaces", ns, "crons", id]) => {
                match self.store.get_cron(ns, id).await {
                    Ok(cron) => (200, json!(cron)),
                    Err(e) => not_found_or_error(e),
                }
            }

            ("PUT", ["v1", "namespaces", ns, "crons", id]) => {
                match self.store.get_cron(ns, id).await {
                    Err(e) => not_found_or_error(e),
                    Ok(mut cron) => {
                        // Updateable fields: schedule, enabled, description,
                        // work_context, concurrency_policy, execution.
                        if let Some(s) = body.get("schedule").and_then(|v| v.as_str()) {
                            cron.schedule = s.to_string();
                        }
                        if let Some(e) = body.get("enabled").and_then(|v| v.as_bool()) {
                            cron.enabled = e;
                        }
                        if let Some(d) = body.get("description").and_then(|v| v.as_str()) {
                            cron.description = d.to_string();
                        }
                        if let Some(ctx) = body.get("work_context").and_then(|v| v.as_object()) {
                            for (k, v) in ctx {
                                cron.work_context.insert(k.clone(), v.clone());
                            }
                        }
                        if let Some(policy) = body.get("concurrency_policy") {
                            match serde_json::from_value(policy.clone()) {
                                Ok(p) => cron.concurrency_policy = p,
                                Err(e) => {
                                    return (
                                        400,
                                        json!({ "error": format!("invalid concurrency_policy: {e}") }),
                                    )
                                }
                            }
                        }
                        cron.updated_at = Utc::now();

                        match self.store.put_cron(&cron).await {
                            Ok(()) => {
                                // Re-index in case enabled status changed.
                                if cron.enabled {
                                    let _ = self.store.index_cron(&cron).await;
                                } else {
                                    let _ =
                                        self.store.deindex_cron(&cron.namespace, &cron.id).await;
                                }
                                (200, json!(cron))
                            }
                            Err(e) => server_error(e),
                        }
                    }
                }
            }

            ("DELETE", ["v1", "namespaces", ns, "crons", id]) => {
                if let Err(e) = self.store.deindex_cron(ns, id).await {
                    log_warn!("Failed to deindex cron '{}/{}': {}", ns, id, e);
                }
                match self.store.delete_cron(ns, id).await {
                    Ok(()) => (204, Value::Null),
                    Err(e) => not_found_or_error(e),
                }
            }

            _ => (404, json!({ "error": "not found" })),
        }
    }

    fn reply(&self, target: &str, corr_id: u64, status: u16, body: Value) {
        let msg = Message::new(
            0,
            "",
            target,
            "http_response",
            json!({ "correlation_id": corr_id, "status": status, "body": body }),
        );
        if self.to_hub.send(msg).is_err() {
            log_warn!("Could not send response — hub disconnected");
        }
    }

    pub(super) fn send_metric(&self, name: &str, operation: &str, value: f64) {
        if self.metrics_target.is_empty() {
            return;
        }
        let msg = Message::new(
            0,
            "",
            &self.metrics_target,
            "update_metric",
            json!({ "name": name, "operation": operation, "value": value }),
        );
        let _ = self.to_hub.send(msg);
    }
}

// ── Error helpers ─────────────────────────────────────────────────────────────

fn not_found_or_error(e: super::store::StoreError) -> (u16, Value) {
    if matches!(e, super::store::StoreError::NotFound(_)) {
        (404, json!({ "error": "not found" }))
    } else {
        server_error(e)
    }
}

fn server_error(e: impl std::fmt::Display) -> (u16, Value) {
    (500, json!({ "error": e.to_string() }))
}
