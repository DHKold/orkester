use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Base document envelope shared by all resource types.
///
/// Mirrors the common schema defined in `schemas/orkester/common/document.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document<Spec, Status = ()> {
    /// Resource kind, e.g. `"orkester/task:1.0"`.
    pub kind: String,
    /// Resource name, unique within its namespace.
    pub name: String,
    /// Namespace, defaults to `"default"`.
    #[serde(default = "default_namespace")]
    pub namespace: String,
    /// SemVer resource version.
    pub version: String,
    /// Free-form metadata (description, tags, author, etc.).
    #[serde(default)]
    pub metadata: DocumentMetadata,
    /// Resource-specific specification.
    pub spec: Spec,
    /// Runtime state; absent for catalog resources, present for execution objects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
}

fn default_namespace() -> String {
    "default".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Arbitrary extra key/value fields (author, team, etc.).
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Raw (untyped) document as loaded from disk before type-specialised parsing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawDocument {
    pub kind: String,
    pub name: String,
    #[serde(default = "default_namespace")]
    pub namespace: String,
    pub version: String,
    #[serde(default)]
    pub metadata: DocumentMetadata,
    pub spec: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<serde_json::Value>,
    /// Original file path (injected after loading, not present in files).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
}

impl RawDocument {
    /// Re-parse this raw document as a typed `Document<Spec, Status>`.
    pub fn into_typed<S, St>(self) -> crate::Result<Document<S, St>>
    where
        S: for<'de> serde::Deserialize<'de>,
        St: for<'de> serde::Deserialize<'de> + Default,
    {
        // Roundtrip through serde_json; `source_path` is ignored (not in Document).
        let v = serde_json::to_value(&self).map_err(crate::WorkaholicError::Json)?;
        serde_json::from_value(v).map_err(crate::WorkaholicError::Json)
    }
}
