//! `LocalWorker` — in-process workflow execution.
//!
//! `run()` is structured as a sequence of named lifecycle phases, each
//! implemented as a private associated function.  This keeps the top-level
//! implementation readable as a high-level narrative while moving error
//! handling and persistence concerns into focused, independently-testable units.
//!
//! # Lifecycle phases
//!
//! ```text
//! start_workflow  →  resolve_work  →  init_steps  →  run_dag  →  finalize
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use orkester_common::domain::Work;
use orkester_common::messaging::Message;
use orkester_common::plugin::providers::executor::ExecutorRegistry;
use serde_json::json;

use super::dag;
use super::super::model::{StepState, StepStatus, Workflow, WorkflowStatus};
use super::super::store::WorkflowsStore;
use super::super::workspace_client::WorkspaceClient;
use super::traits::Worker;

// ── Struct ────────────────────────────────────────────────────────────────────

pub struct LocalWorker {
    pub executor_registry: Arc<ExecutorRegistry>,
    pub to_hub: std::sync::mpsc::Sender<Message>,
    pub metrics_target: String,
}

// ── Worker trait impl ─────────────────────────────────────────────────────────

#[async_trait]
impl Worker for LocalWorker {
    async fn run(&self, mut workflow: Workflow, store: WorkflowsStore, workspace: WorkspaceClient) {
        orkester_common::log_info!(
            "Worker: starting workflow '{}' ({}/{}@{})",
            workflow.id,
            workflow.namespace,
            workflow.work_name,
            workflow.work_version,
        );

        if Self::start_workflow(&mut workflow, &store).await.is_err() {
            return;
        }

        // Workflow transitioned to Running — emit started + active metrics.
        self.send_metric("workflows.started_total", "increment", 1.0);
        self.send_metric("workflows.active", "increment", 1.0);

        let work = match Self::resolve_work(&workflow, &workspace).await {
            Ok(w) => w,
            Err(reason) => {
                dag::fail_workflow(&mut workflow, reason, &store).await;
                self.send_metric("workflows.failed_total", "increment", 1.0);
                self.send_metric("workflows.active", "increment", -1.0);
                return;
            }
        };

        if Self::init_steps(&mut workflow, &work, &store).await.is_err() {
            self.send_metric("workflows.failed_total", "increment", 1.0);
            self.send_metric("workflows.active", "increment", -1.0);
            return;
        }

        let result = self.run_dag(&mut workflow, &work, &store, &workspace).await;

        Self::finalize(&mut workflow, result, &store).await;

        // Emit the terminal-state metric.
        match workflow.status {
            WorkflowStatus::Succeeded => {
                self.send_metric("workflows.succeeded_total", "increment", 1.0);
            }
            WorkflowStatus::Cancelled => {
                self.send_metric("workflows.cancelled_total", "increment", 1.0);
            }
            _ => {
                self.send_metric("workflows.failed_total", "increment", 1.0);
            }
        }
        self.send_metric("workflows.active", "increment", -1.0);
    }

    async fn cancel(&self, workflow_id: &str, namespace: &str, store: WorkflowsStore) {
        match store.get_workflow(namespace, workflow_id).await {
            Ok(mut wf) if !wf.status.is_terminal() => {
                wf.status = WorkflowStatus::Cancelled;
                wf.finished_at = Some(Utc::now());
                wf.updated_at = Utc::now();
                if let Err(e) = store.put_workflow(&wf).await {
                    orkester_common::log_error!(
                        "Worker: failed to persist Cancelled state for '{}': {}",
                        workflow_id,
                        e,
                    );
                } else {
                    self.send_metric("workflows.cancelled_total", "increment", 1.0);
                    self.send_metric("workflows.active", "increment", -1.0);
                }
            }
            Ok(wf) => orkester_common::log_warn!(
                "Worker: workflow '{}' is already terminal ({}), ignoring cancel.",
                workflow_id,
                wf.status,
            ),
            Err(e) => orkester_common::log_error!(
                "Worker: workflow '{}' not found during cancel: {}",
                workflow_id,
                e,
            ),
        }
    }
}

// ── Private lifecycle phases ──────────────────────────────────────────────────

impl LocalWorker {
    fn send_metric(&self, name: &str, operation: &str, value: f64) {
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

    /// Transition the workflow to `Running` and persist the initial state.
    async fn start_workflow(workflow: &mut Workflow, store: &WorkflowsStore) -> Result<(), ()> {
        workflow.status = WorkflowStatus::Running;
        workflow.started_at = Some(Utc::now());
        workflow.updated_at = Utc::now();
        store.put_workflow(workflow).await.map_err(|e| {
            orkester_common::log_error!(
                "Worker: failed to persist Running state for '{}': {}",
                workflow.id,
                e,
            );
        })
    }

    /// Fetch the `Work` definition from the workspace.
    ///
    /// Returns `Err(reason)` (without touching the store) if the lookup fails;
    /// the caller is responsible for calling [`dag::fail_workflow`].
    async fn resolve_work(workflow: &Workflow, workspace: &WorkspaceClient) -> Result<Work, String> {
        workspace
            .get_work(&workflow.namespace, &workflow.work_name, &workflow.work_version)
            .await
            .map_err(|e| format!("could not fetch Work definition: {e}"))
    }

    /// Seed a `Pending` [`StepState`] for every step defined in `work` and
    /// persist the initialised workflow.
    async fn init_steps(
        workflow: &mut Workflow,
        work: &Work,
        store: &WorkflowsStore,
    ) -> Result<(), ()> {
        workflow.metrics.steps_total = work.spec.steps.len() as u32;
        for step in &work.spec.steps {
            workflow.steps.insert(
                step.id.clone(),
                StepState {
                    status: StepStatus::Pending,
                    started_at: None,
                    finished_at: None,
                    outputs: HashMap::new(),
                    error: None,
                    attempt: 1,
                },
            );
        }
        store.put_workflow(workflow).await.map_err(|e| {
            orkester_common::log_error!(
                "Worker: failed to persist initial step states for '{}': {}",
                workflow.id,
                e,
            );
        })
    }

    /// Run the DAG, wrapping it in an optional whole-workflow timeout.
    async fn run_dag(
        &self,
        workflow: &mut Workflow,
        work: &Work,
        store: &WorkflowsStore,
        workspace: &WorkspaceClient,
    ) -> Result<(), String> {
        let executors = Arc::clone(&self.executor_registry);
        match workflow.execution.timeout_seconds.map(Duration::from_secs) {
            Some(t) => tokio::time::timeout(
                t,
                dag::execute(workflow, work, store, workspace, &executors),
            )
            .await
            .unwrap_or_else(|_| Err("workflow timed out".to_string())),
            None => dag::execute(workflow, work, store, workspace, &executors).await,
        }
    }

    /// Set the terminal status, record duration, log the outcome, and persist.
    async fn finalize(workflow: &mut Workflow, result: Result<(), String>, store: &WorkflowsStore) {
        let now = Utc::now();
        workflow.status = if result.is_ok() {
            WorkflowStatus::Succeeded
        } else {
            WorkflowStatus::Failed
        };
        workflow.finished_at = Some(now);
        workflow.updated_at = now;
        if let Some(start) = workflow.started_at {
            workflow.metrics.duration_seconds =
                Some((now - start).num_milliseconds() as f64 / 1000.0);
        }

        if let Err(ref reason) = result {
            orkester_common::log_error!("Worker: workflow '{}' failed — {}", workflow.id, reason);
        }
        orkester_common::log_info!(
            "Worker: workflow '{}' finished as {}",
            workflow.id,
            workflow.status,
        );

        if let Err(e) = store.put_workflow(workflow).await {
            orkester_common::log_error!(
                "Worker: failed to persist final state for '{}': {}",
                workflow.id,
                e,
            );
        }
    }
}
