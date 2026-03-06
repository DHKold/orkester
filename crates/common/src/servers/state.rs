use std::sync::Arc;
use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;
use crate::domain::{Workspace, WorkspaceId, Work, WorkId};
use crate::servers::ServerContext;

#[derive(Debug, Error)]
pub enum StateError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Already exists: {0}")]
    AlreadyExists(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Interface for reading and writing platform state (Workspaces, Works, Tasks).
/// All other servers and services interact with state exclusively through this handle.
#[async_trait]
pub trait StateHandle: Send + Sync {
    // ── Workspaces ─────────────────────────────────────────────────────────
    async fn list_workspaces(&self) -> Result<Vec<Workspace>, StateError>;
    async fn get_workspace(&self, id: &WorkspaceId) -> Result<Workspace, StateError>;
    async fn create_workspace(&self, workspace: Workspace) -> Result<(), StateError>;
    async fn delete_workspace(&self, id: &WorkspaceId) -> Result<(), StateError>;

    // ── Works ──────────────────────────────────────────────────────────────
    async fn list_works(&self, workspace_id: &WorkspaceId) -> Result<Vec<Work>, StateError>;
    async fn get_work(&self, workspace_id: &WorkspaceId, work_id: &WorkId) -> Result<Work, StateError>;
    async fn put_work(&self, work: Work) -> Result<(), StateError>;
    async fn delete_work(&self, workspace_id: &WorkspaceId, work_id: &WorkId) -> Result<(), StateError>;
}

/// A running State server. `run()` drives the server until shutdown.
#[async_trait]
pub trait StateServer: Send + Sync {
    fn name(&self) -> &str;
    /// Returns the handle for use by other components.
    fn handle(&self) -> Arc<dyn StateHandle>;
    /// Drive the server. Resolves when the server shuts down.
    fn run(self: Box<Self>) -> ServerContext<(), ()>;
}

/// Plugin factory for creating a StateServer from configuration.
pub trait StateServerFactory: Send + Sync {
    fn name(&self) -> &str;
    fn build(&self, config: Value) -> Result<Box<dyn StateServer>, StateError>;
}
