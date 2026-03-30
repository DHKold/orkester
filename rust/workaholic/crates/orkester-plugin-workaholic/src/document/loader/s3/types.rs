//! Data types shared across all `s3` submodules.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use workaholic::Document;

// ─── Loader config ────────────────────────────────────────────────────────────

/// Top-level config for the S3 loader (extension → parser mapping).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct S3LoaderConfig {
    /// File extension → parser kind, e.g. `{"yaml": "yaml", "json": "json"}`.
    #[serde(default)]
    pub extensions: HashMap<String, String>,
}

/// Config for one watched S3 bucket/prefix entry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct S3LoaderEntryConfig {
    pub bucket:             String,
    #[serde(default)]
    pub prefix:             String,
    #[serde(default = "default_region")]
    pub region:             String,
    /// Override endpoint URL (e.g. `http://minio:9000` for MinIO).
    #[serde(default)]
    pub endpoint_url:       Option<String>,
    #[serde(default)]
    pub access_key_id:      Option<String>,
    #[serde(default)]
    pub secret_access_key:  Option<String>,
    #[serde(default)]
    pub recursive:          bool,
    #[serde(default)]
    pub watch:              bool,
    #[serde(default = "default_poll")]
    pub poll_interval_secs: u64,
}

fn default_region() -> String { "us-east-1".into() }
fn default_poll()   -> u64    { 30 }

// ─── Runtime entry state ──────────────────────────────────────────────────────

/// A single watched S3 bucket/prefix, with cached object state.
pub struct S3Entry {
    pub config:          S3LoaderEntryConfig,
    /// key → (etag, documents). Etag is used for change detection.
    pub loaded_objects:  HashMap<String, S3LoadedObject>,
}

/// Snapshot of one S3 object at last parse time.
pub struct S3LoadedObject {
    pub key:       String,
    pub etag:      String,
    pub documents: Vec<Document>,
}

// ─── Change events ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum S3ChangeEvent {
    DocumentAdded    { entry_id: String, key: String, document: Document },
    DocumentModified { entry_id: String, key: String, old: Document, new: Document },
    DocumentRemoved  { entry_id: String, key: String, document: Document },
}

// ─── Scan metrics ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3ScanMetrics {
    pub entry_id:        String,
    pub scanned_at:      String,
    pub is_initial:      bool,
    pub duration_ms:     u64,
    pub events_added:    usize,
    pub events_modified: usize,
    pub events_removed:  usize,
}
