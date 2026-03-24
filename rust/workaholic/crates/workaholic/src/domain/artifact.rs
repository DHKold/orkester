use serde::{Deserialize, Serialize};

use crate::document::Document;

pub type Artifact = Document<ArtifactSpec>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArtifactSpec {
    /// Reference to the WorkRun or TaskRun that produced this artifact.
    pub owner_ref: String,
    /// URI where the artifact data is stored.
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<ArtifactChecksum>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactChecksum {
    pub sha256: String,
}
