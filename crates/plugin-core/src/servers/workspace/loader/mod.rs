//! Object loader abstraction and implementations.
//!
//! A loader is responsible for:
//! 1. Discovering YAML files (from a directory, S3 prefix, etc.)
//! 2. Parsing multi-document YAML into [`ObjectEnvelope`]s
//! 3. Detecting changes at runtime and emitting [`LoaderEvent`]s

pub mod local;
pub mod s3;

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use orkester_common::domain::ObjectEnvelope;

// ── Events ────────────────────────────────────────────────────────────────────

/// An event emitted by a loader when the source changes.
#[derive(Debug, Clone)]
pub enum LoaderEvent {
    /// A new object was discovered, or an existing one was updated.
    Upserted(ObjectEnvelope),
    /// An object that was previously known has been removed (either its source
    /// file was deleted, or the document was removed from the file).
    Removed(ObjectEnvelope),
}

// ── ObjectLoader trait ────────────────────────────────────────────────────────

/// Implemented by every object source (local directory, S3 prefix, etc.)
#[async_trait]
pub trait ObjectLoader: Send + Sync {
    /// Perform the initial load, returning all objects currently available.
    async fn load_all(&self) -> Result<Vec<ObjectEnvelope>, LoaderError>;

    /// Start a background change-detection task that sends events on `tx`.
    ///
    /// The task should run until the sender is dropped or the loader is shut
    /// down.  This method is called once per loader, soon after `load_all`.
    async fn watch(&self, tx: tokio::sync::mpsc::UnboundedSender<LoaderEvent>);
}

// ── Loader error ─────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum LoaderError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml parse error in '{path}': {source}")]
    Yaml {
        path: String,
        source: serde_yaml::Error,
    },
    #[error("configuration error: {0}")]
    Config(String),
}

// ── YAML parsing helpers (shared) ────────────────────────────────────────────

/// Parse all YAML documents from `content` (a multi-document YAML string)
/// that was loaded from `path` (used only in error messages).
pub fn parse_yaml_documents(content: &str, path: &str) -> Result<Vec<ObjectEnvelope>, LoaderError> {
    let mut objects = Vec::new();
    for doc in serde_yaml::Deserializer::from_str(content) {
        let value: serde_yaml::Value =
            serde_yaml::Value::deserialize(doc).map_err(|e| LoaderError::Yaml {
                path: path.to_string(),
                source: e,
            })?;

        // Skip empty / null documents (e.g. trailing `---`)
        if value.is_null() {
            continue;
        }

        // Manually dispatch on `kind` instead of relying on serde's
        // internally-tagged enum, which conflicts with #[serde(flatten)] in
        // serde_yaml — the tag field is consumed and not forwarded to the
        // flattened ObjectMeta, causing a "missing field `kind`" error.
        let kind = value.get("kind").and_then(|v| v.as_str()).ok_or_else(|| {
            LoaderError::Config(format!("document in '{}' is missing a 'kind' field", path))
        })?;

        let envelope = match kind {
            "Namespace" => {
                ObjectEnvelope::Namespace(serde_yaml::from_value(value).map_err(|e| {
                    LoaderError::Yaml {
                        path: path.to_string(),
                        source: e,
                    }
                })?)
            }
            "Task" => ObjectEnvelope::Task(serde_yaml::from_value(value).map_err(|e| {
                LoaderError::Yaml {
                    path: path.to_string(),
                    source: e,
                }
            })?),
            "Work" => ObjectEnvelope::Work(serde_yaml::from_value(value).map_err(|e| {
                LoaderError::Yaml {
                    path: path.to_string(),
                    source: e,
                }
            })?),
            other => {
                return Err(LoaderError::Config(format!(
                    "unknown kind '{}' in '{}'",
                    other, path
                )))
            }
        };
        objects.push(envelope);
    }
    Ok(objects)
}

/// Build a loader from a `serde_json::Value` config block.
///
/// The config must contain `type: "local"` or `type: "s3"` plus
/// type-specific fields.
pub fn loader_from_config(cfg: &serde_json::Value) -> Result<Arc<dyn ObjectLoader>, LoaderError> {
    let loader_type = cfg
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| LoaderError::Config("loader config missing 'type' field".into()))?;

    match loader_type {
        "local" => {
            let dir = cfg
                .get("dir")
                .and_then(|v| v.as_str())
                .ok_or_else(|| LoaderError::Config("local loader requires 'dir'".into()))?;
            Ok(Arc::new(local::LocalLoader::new(dir)))
        }
        "s3" => {
            let bucket = cfg
                .get("bucket")
                .and_then(|v| v.as_str())
                .ok_or_else(|| LoaderError::Config("s3 loader requires 'bucket'".into()))?;
            let prefix = cfg.get("prefix").and_then(|v| v.as_str()).unwrap_or("");
            let poll_secs = cfg
                .get("poll_interval_seconds")
                .and_then(|v| v.as_u64())
                .unwrap_or(60);
            Ok(Arc::new(s3::S3Loader::new(bucket, prefix, poll_secs)))
        }
        other => Err(LoaderError::Config(format!(
            "unknown loader type '{other}'"
        ))),
    }
}
