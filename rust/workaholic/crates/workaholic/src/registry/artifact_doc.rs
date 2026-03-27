use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::document::Document;

pub const ARTIFACT_KIND: &str = "workaholic/Artifact:1.0";

pub type ArtifactDoc = Document<ArtifactSpec, ArtifactStatus>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArtifactSpec {
    /// Semantic type of the artifact (e.g. `file`, `folder`, `structured`).
    #[serde(rename = "type")]
    pub artifact_type: String,
    /// Registry kind that stores this artifact (e.g. `local-filesystem`).
    pub registry: String,
    /// Registry-specific properties (URI, checksum, size, etc.).
    #[serde(default)]
    pub properties: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArtifactStatus {
    #[serde(default)]
    pub deleted: bool,
    #[serde(rename = "createdAt", default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(rename = "updatedAt", default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(rename = "lastUsedAt", default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
}
