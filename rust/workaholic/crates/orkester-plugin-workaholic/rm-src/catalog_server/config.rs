use serde::{Deserialize, Serialize};

/// Configuration passed to `CatalogServer` at creation time.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CatalogServerConfig {
    /// Optional path (file or directory) from which to load initial documents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader: Option<LoaderConfig>,
    /// Whether to load documents at startup.
    #[serde(default = "default_true")]
    pub load_on_startup: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoaderConfig {
    /// Local filesystem path.
    pub path: String,
}
