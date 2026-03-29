use serde_json::Value;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct ResourceCreationRequest {
    pub id: String,
    pub resource: Value,
}

#[derive(Deserialize)]
pub struct ResourceRetrievalRequest {
    pub id: String,
}

#[derive(Deserialize)]
pub struct ResourceUpdateRequest {
    pub id: String,
    pub resource: Value,
}

#[derive(Deserialize)]
pub struct ResourceDeletionRequest {
    pub id: String,
}

#[derive(Deserialize)]
pub struct ResourceSearchRequest {
    pub query: String,
}

#[derive(Deserialize)]
pub struct CatalogLoadDocumentsRequest{
    pub loader_ref: String,
}

// ── List requests / responses ─────────────────────────────────────────────────

/// Used by ListWorks and ListTasks — filters by namespace name.
#[derive(Deserialize)]
pub struct ListItemsRequest {
    /// Namespace name to filter by (matches `metadata.namespace` in the document).
    pub ns: String,
}

#[derive(Serialize)]
pub struct ListNamespacesResponse {
    pub namespaces: Vec<Value>,
}

#[derive(Serialize)]
pub struct ListWorksResponse {
    pub works: Vec<Value>,
}

#[derive(Serialize)]
pub struct ListTasksResponse {
    pub tasks: Vec<Value>,
}
