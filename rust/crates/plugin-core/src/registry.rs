use async_trait::async_trait;
use orkester_common::plugin::providers::registry::{
    RegistryBuilder, RegistryError, WorkflowDefinition, WorkflowRegistry,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// An in-memory workflow registry. Workflows can be loaded from a local directory
/// at startup (if `path` is provided in config) or managed purely via API.
///
/// # Configuration (JSON)
/// ```json
/// {
///   "path": "./workflows"   // optional: directory of JSON/YAML workflow files to load at startup
/// }
/// ```
#[derive(Default)]
pub struct LocalWorkflowRegistry {
    store: Arc<RwLock<HashMap<String, WorkflowDefinition>>>,
}

#[async_trait]
impl WorkflowRegistry for LocalWorkflowRegistry {
    async fn list_workflows(&self) -> Result<Vec<WorkflowDefinition>, RegistryError> {
        let store = self.store.read().await;
        Ok(store.values().cloned().collect())
    }

    async fn get_workflow(&self, id: &str) -> Result<WorkflowDefinition, RegistryError> {
        let store = self.store.read().await;
        store
            .get(id)
            .cloned()
            .ok_or_else(|| RegistryError::NotFound(id.to_string()))
    }

    async fn put_workflow(
        &self,
        id: &str,
        definition: WorkflowDefinition,
    ) -> Result<(), RegistryError> {
        let mut store = self.store.write().await;
        tracing::debug!(id = %id, "LocalWorkflowRegistry: put_workflow");
        store.insert(id.to_string(), definition);
        Ok(())
    }

    async fn delete_workflow(&self, id: &str) -> Result<(), RegistryError> {
        let mut store = self.store.write().await;
        if store.remove(id).is_none() {
            return Err(RegistryError::NotFound(id.to_string()));
        }
        Ok(())
    }
}

pub struct LocalRegistryBuilder;

impl RegistryBuilder for LocalRegistryBuilder {
    fn build(&self, _config: Value) -> Result<Box<dyn WorkflowRegistry>, RegistryError> {
        // TODO: if config["path"] is set, scan the directory and pre-populate the store.
        Ok(Box::new(LocalWorkflowRegistry::default()))
    }
}
