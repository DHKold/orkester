use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Already exists: {0}")]
    AlreadyExists(String),
    #[error("Configuration error: {0}")]
    Configuration(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

/// A raw workflow definition as loaded from any source (JSON or YAML deserialized).
pub type WorkflowDefinition = Value;

/// Trait that all WorkflowRegistry implementations must satisfy.
///
/// A registry is the source-of-truth for workflow (Works/Tasks) definitions.
#[async_trait]
pub trait WorkflowRegistry: Send + Sync {
    /// Return all workflow definitions available in this registry.
    async fn list_workflows(&self) -> Result<Vec<WorkflowDefinition>, RegistryError>;

    /// Fetch a single workflow by its ID.
    async fn get_workflow(&self, id: &str) -> Result<WorkflowDefinition, RegistryError>;

    /// Store or update a workflow definition.
    async fn put_workflow(
        &self,
        id: &str,
        definition: WorkflowDefinition,
    ) -> Result<(), RegistryError>;

    /// Delete a workflow by ID.
    async fn delete_workflow(&self, id: &str) -> Result<(), RegistryError>;
}

/// Builder that creates a [`WorkflowRegistry`] from a JSON configuration.
pub trait RegistryBuilder: Send + Sync {
    fn build(&self, config: Value) -> Result<Box<dyn WorkflowRegistry>, RegistryError>;
}
