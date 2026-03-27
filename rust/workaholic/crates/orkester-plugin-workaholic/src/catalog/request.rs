use serde_json::Value;
use serde::Deserialize;

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