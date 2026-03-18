use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Describes a single component kind that a plugin can produce.
///
/// This is the canonical wire type for component-kind metadata.  Plugins
/// serialize it in their `Metadata` response; hosts deserialize it to
/// discover what a plugin can do.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentKind {
    pub id: u32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub extra: Map<String, Value>,
}

impl ComponentKind {
    /// Convenience constructor for the common case: id + name + description.
    pub fn new(id: u32, name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id,
            name: Some(name.into()),
            description: Some(description.into()),
            tags: Vec::new(),
            extra: Map::new(),
        }
    }
}