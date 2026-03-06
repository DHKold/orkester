use std::collections::HashMap;
use std::sync::{Arc, mpsc};
use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::RwLock;
use tracing::info;
use orkester_common::domain::{Workspace, WorkspaceId, Work, WorkId};
use orkester_common::servers::ServerContext;
use orkester_common::servers::state::{StateError, StateHandle, StateServer, StateServerFactory};

// ── Inner state ───────────────────────────────────────────────────────────────

struct Inner {
    workspaces: HashMap<WorkspaceId, Workspace>,
    works: HashMap<(WorkspaceId, WorkId), Work>,
}

impl Inner {
    fn new() -> Self {
        Self {
            workspaces: HashMap::new(),
            works: HashMap::new(),
        }
    }
}

// ── Handle ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct BasicStateHandle(Arc<RwLock<Inner>>);

#[async_trait]
impl StateHandle for BasicStateHandle {
    async fn list_workspaces(&self) -> Result<Vec<Workspace>, StateError> {
        Ok(self.0.read().await.workspaces.values().cloned().collect())
    }

    async fn get_workspace(&self, id: &WorkspaceId) -> Result<Workspace, StateError> {
        self.0
            .read()
            .await
            .workspaces
            .get(id)
            .cloned()
            .ok_or_else(|| StateError::NotFound(id.0.clone()))
    }

    async fn create_workspace(&self, workspace: Workspace) -> Result<(), StateError> {
        let mut g = self.0.write().await;
        if g.workspaces.contains_key(&workspace.id) {
            return Err(StateError::AlreadyExists(workspace.id.0.clone()));
        }
        g.workspaces.insert(workspace.id.clone(), workspace);
        Ok(())
    }

    async fn delete_workspace(&self, id: &WorkspaceId) -> Result<(), StateError> {
        let mut g = self.0.write().await;
        g.workspaces
            .remove(id)
            .ok_or_else(|| StateError::NotFound(id.0.clone()))?;
        // Cascade-delete all works in this workspace
        g.works.retain(|(ws_id, _), _| ws_id != id);
        Ok(())
    }

    async fn list_works(&self, workspace_id: &WorkspaceId) -> Result<Vec<Work>, StateError> {
        let g = self.0.read().await;
        Ok(g.works
            .iter()
            .filter(|((ws_id, _), _)| ws_id == workspace_id)
            .map(|(_, v)| v.clone())
            .collect())
    }

    async fn get_work(
        &self,
        workspace_id: &WorkspaceId,
        work_id: &WorkId,
    ) -> Result<Work, StateError> {
        self.0
            .read()
            .await
            .works
            .get(&(workspace_id.clone(), work_id.clone()))
            .cloned()
            .ok_or_else(|| StateError::NotFound(work_id.0.clone()))
    }

    async fn put_work(&self, work: Work) -> Result<(), StateError> {
        let mut g = self.0.write().await;
        g.works.insert((work.workspace_id.clone(), work.id.clone()), work);
        Ok(())
    }

    async fn delete_work(
        &self,
        workspace_id: &WorkspaceId,
        work_id: &WorkId,
    ) -> Result<(), StateError> {
        self.0
            .write()
            .await
            .works
            .remove(&(workspace_id.clone(), work_id.clone()))
            .ok_or_else(|| StateError::NotFound(work_id.0.clone()))?;
        Ok(())
    }
}

// ── Server ────────────────────────────────────────────────────────────────────

pub struct BasicStateServer {
    handle: BasicStateHandle,
}

#[async_trait]
impl StateServer for BasicStateServer {
    fn name(&self) -> &str {
        "basic-state-server"
    }

    fn handle(&self) -> Arc<dyn StateHandle> {
        Arc::new(self.handle.clone())
    }

    fn run(self: Box<Self>) -> ServerContext<(), ()> {
        let (h2s_sender, h2s_receiver) = mpsc::channel();
        let (s2h_sender, s2h_receiver) = mpsc::channel();
        let hd = std::thread::spawn(move || {
            // Pure in-memory: no background work needed — just keep alive.
            info!("BasicStateServer running (in-memory)");
            h2s_receiver.recv().ok();
            s2h_sender.send(()).ok();
        });
        ServerContext {
            receiver: Some(s2h_receiver),
            sender: Some(h2s_sender),
            handle: hd,
        }
    }
}

// ── Factory ───────────────────────────────────────────────────────────────────

pub struct BasicStateServerFactory;

impl StateServerFactory for BasicStateServerFactory {
    fn name(&self) -> &str {
        "basic-state-server"
    }

    fn build(&self, _config: Value) -> Result<Box<dyn StateServer>, StateError> {
        Ok(Box::new(BasicStateServer {
            handle: BasicStateHandle(Arc::new(RwLock::new(Inner::new()))),
        }))
    }
}
