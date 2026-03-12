//! Worker — responsible for actually executing a Workflow step by step.
//!
//! # Architecture (planned)
//!
//! ```
//!  WorkflowsServer
//!       │
//!       ├── Scheduler loop  (fires Crons, creates Workflow records)
//!       │
//!       └── Worker pool     (picks up Waiting/Running Workflows and drives them)
//!               │
//!               └── TaskExecutor  (runs individual Tasks via the executor plugin)
//! ```
//!
//! The [`Worker`] trait below defines the contract.  The single concrete
//! implementation provided here is [`LocalWorker`], which runs tasks
//! sequentially in the same process.  Real implementations (k8s job worker,
//! remote agent, etc.) can be added later by implementing the same trait.

use async_trait::async_trait;

use super::model::{StepState, StepStatus, Workflow, WorkflowStatus};
use super::store::{StoreResult, WorkflowsStore};
use super::workspace_client::WorkspaceClient;

// ── Worker trait ──────────────────────────────────────────────────────────────

/// Responsible for driving a [`Workflow`] from `Waiting` → terminal state.
///
/// Implementors are expected to:
/// 1. Resolve step dependencies (the DAG defined in the Work).
/// 2. Execute each ready step via the appropriate [`TaskExecutor`].
/// 3. Persist step-level and workflow-level state changes back to the
///    [`WorkflowsStore`] as they occur.
/// 4. Respect [`WorkflowExecution`] policy (timeouts, failure_policy, etc.).
#[async_trait]
pub trait Worker: Send + Sync {
    /// Drive `workflow` to completion (or failure/cancellation).
    ///
    /// This method is expected to update the workflow record in the store
    /// as it progresses (status, step states, metrics, timestamps).
    /// It should return once the workflow reaches a terminal state.
    ///
    /// `workspace` allows the worker to resolve `Work` and `Task` definitions
    /// from the Workspace server on demand.
    async fn run(&self, workflow: Workflow, store: WorkflowsStore, workspace: WorkspaceClient);

    /// Request a graceful cancellation of the given workflow.
    ///
    /// The implementation should attempt to stop in-flight task executions
    /// and transition the workflow to [`WorkflowStatus::Cancelled`].
    async fn cancel(&self, workflow_id: &str, namespace: &str, store: WorkflowsStore);
}

// ── LocalWorker ───────────────────────────────────────────────────────────────

/// In-process worker implementation.
///
/// Currently a skeleton — task execution is stubbed with TODOs.
/// Replace the TODO sections with real [`TaskExecutor`] dispatch.
pub struct LocalWorker;

#[async_trait]
impl Worker for LocalWorker {
    async fn run(&self, mut workflow: Workflow, store: WorkflowsStore, workspace: WorkspaceClient) {
        use chrono::Utc;

        orkester_common::log_info!(
            "Worker: starting workflow '{}' ({}/{})",
            workflow.id,
            workflow.work_name,
            workflow.work_version
        );

        workflow.status = WorkflowStatus::Running;
        workflow.started_at = Some(Utc::now());
        workflow.updated_at = Utc::now();

        if let Err(e) = store.put_workflow(&workflow).await {
            orkester_common::log_error!(
                "Worker: failed to persist status for workflow '{}': {}",
                workflow.id,
                e
            );
            return;
        }

        // TODO: Resolve Work and Task definitions using `workspace.get_work()` and
        //       `workspace.get_task()` to build the full step DAG.

        // TODO: Build a dependency graph from Work.spec.steps and iterate
        //       in topological order.

        // TODO: For each ready step:
        //   1. Resolve the Task definition.
        //   2. Select the appropriate TaskExecutor based on task.spec.executor.
        //   3. Call executor.execute(task, inputs).await.
        //   4. Update StepState (status, outputs, error, timing).
        //   5. Persist the updated Workflow record.
        //   6. Apply WorkflowExecution.failure_policy if the step failed.

        // TODO: Respect WorkflowExecution.timeout_seconds via tokio::time::timeout.

        // TODO: Handle WorkflowStatus::Paused transitions (external signal via hub).

        // For now, immediately mark the workflow as Succeeded as a stub.
        orkester_common::log_warn!(
            "Worker: execution not yet implemented — marking workflow '{}' as Succeeded (stub).",
            workflow.id
        );

        workflow.status = WorkflowStatus::Succeeded;
        workflow.finished_at = Some(Utc::now());
        workflow.updated_at = Utc::now();
        workflow.metrics.steps_total = 0;

        if let Err(e) = store.put_workflow(&workflow).await {
            orkester_common::log_error!(
                "Worker: failed to persist final status for workflow '{}': {}",
                workflow.id,
                e
            );
        }
    }

    async fn cancel(&self, workflow_id: &str, namespace: &str, store: WorkflowsStore) {
        use chrono::Utc;

        match store.get_workflow(namespace, workflow_id).await {
            Ok(mut wf) => {
                if wf.status.is_terminal() {
                    orkester_common::log_warn!(
                        "Worker: workflow '{}' is already terminal ({}), ignoring cancel.",
                        workflow_id,
                        wf.status
                    );
                    return;
                }

                // TODO: Signal any running TaskExecutor to abort the in-flight task.

                wf.status = WorkflowStatus::Cancelled;
                wf.finished_at = Some(Utc::now());
                wf.updated_at = Utc::now();

                if let Err(e) = store.put_workflow(&wf).await {
                    orkester_common::log_error!(
                        "Worker: failed to persist Cancelled status for workflow '{}': {}",
                        workflow_id,
                        e
                    );
                }
            }
            Err(e) => {
                orkester_common::log_error!(
                    "Worker: workflow '{}' not found during cancel: {}",
                    workflow_id,
                    e
                );
            }
        }
    }
}
