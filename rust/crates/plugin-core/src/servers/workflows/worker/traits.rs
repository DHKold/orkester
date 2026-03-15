//! Public `Worker` trait.

use async_trait::async_trait;

use super::super::model::Workflow;
use super::super::store::WorkflowsStore;
use super::super::workspace_client::WorkspaceClient;

/// Drives a [`Workflow`] to completion (success, failure, or cancellation).
#[async_trait]
pub trait Worker: Send + Sync {
    /// Execute `workflow` to completion.  The workflow object is consumed;
    /// all state transitions are persisted to `store`.
    async fn run(&self, workflow: Workflow, store: WorkflowsStore, workspace: WorkspaceClient);

    /// Mark the given workflow as `Cancelled`.
    ///
    /// The running DAG loop will notice on its next wave boundary and stop
    /// launching new steps.
    async fn cancel(&self, workflow_id: &str, namespace: &str, store: WorkflowsStore);
}
