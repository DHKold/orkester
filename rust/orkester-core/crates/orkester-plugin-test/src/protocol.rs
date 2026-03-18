use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateComponentRequest {
    pub component_id: String,
    pub config: serde_json::Value,
}