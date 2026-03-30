//! Data types shared across all `local_fs` submodules.

use std::collections::HashMap;
use std::time::SystemTime;

use workaholic::Document;

// ─── Change events ─────────────────────────────────────────────────────────────

/// Emitted whenever the set of documents reachable from a watched path changes.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LocalFsChangeEvent {
    /// A document was discovered for the first time.
    DocumentAdded {
        /// The entry `path` that owns this document.
        entry_path: String,
        /// Absolute path of the file the document was loaded from.
        source_path: String,
        document: Document,
    },
    /// A document already present in the cache changed (version, spec, or metadata).
    DocumentModified {
        entry_path:  String,
        source_path: String,
        old: Document,
        new: Document,
    },
    /// A document that was previously loaded is no longer present on disk.
    DocumentRemoved {
        entry_path:  String,
        source_path: String,
        document:    Document,
    },
}

// ─── Scan metrics ─────────────────────────────────────────────────────────────

/// Timing and event counters recorded for a single scan of one watched entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LocalFsScanMetrics {
    /// Root path of the entry that was scanned.
    pub entry_path:      String,
    /// ISO 8601 timestamp when this scan started.
    pub scanned_at:      String,
    /// Whether this was the initial startup scan (`true`) or a background poll (`false`).
    pub is_initial:      bool,
    /// Wall-clock time spent performing the scan in milliseconds.
    pub duration_ms:     u64,
    /// Number of `DocumentAdded` events produced.
    pub events_added:    usize,
    /// Number of `DocumentModified` events produced.
    pub events_modified: usize,
    /// Number of `DocumentRemoved` events produced.
    pub events_removed:  usize,
}

// ─── Runtime state ─────────────────────────────────────────────────────────────

/// A single watched root path together with its cached document state.
///
/// Modified in-place by the scanner and background watcher.
pub struct LocalFsEntry {
    /// Root path to load from (file or directory).
    pub path: String,
    /// When `true` and `path` is a directory, recurse into sub-directories.
    pub recursive: bool,
    /// When `true`, a background polling thread watches this entry for changes.
    pub watch: bool,
    /// Last-parsed state of every discovered file, keyed by absolute file path.
    pub loaded_files: HashMap<String, LocalFsLoadedFile>,
}

/// Snapshot of a single file at last parse time.
pub struct LocalFsLoadedFile {
    /// Absolute path of the file.
    pub path: String,
    /// The `mtime` at the time this file was last parsed. Used for change detection.
    pub last_parsed: SystemTime,
    /// Every document the file contained at last parse time.
    pub documents: Vec<Document>,
}

// ─── Configuration DTOs ────────────────────────────────────────────────────────

/// Top-level component configuration, supplied when the component is constructed.
#[derive(serde::Deserialize)]
pub struct LocalFsLoaderConfig {
    /// Maps file extension (e.g. `"yaml"`) to parser kind (e.g. `"yaml"`).
    #[serde(default)]
    pub extensions: HashMap<String, String>,
}

/// Per-entry configuration exposed through the component interface (create / update / retrieve).
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct LocalFsLoaderEntryConfig {
    /// Root path to load documents from.
    pub path: String,
    /// Recurse into subdirectories.
    #[serde(default)]
    pub recursive: bool,
    /// Enable background change-watching.
    #[serde(default)]
    pub watch: bool,
}
