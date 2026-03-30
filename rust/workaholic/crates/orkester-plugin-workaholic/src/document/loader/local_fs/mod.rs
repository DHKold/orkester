//! Local filesystem document loader.
//!
//! # Structure
//!
//! | Module        | Responsibility                                               |
//! |---------------|--------------------------------------------------------------|
//! | `types`       | All shared data types (events, entries, configs)             |
//! | `fs`          | Raw filesystem ops: file collection and document parsing     |
//! | `diff`        | Document identity, equality checks, and change-event helpers |
//! | `scanner`     | Full entry scan orchestrating `fs` + `diff`                  |
//! | `watcher`     | Background polling thread                                    |
//! | `component`   | Orkester component wrapper                                   |
//!
//! The public surface is [`LocalFsLoader`] (for programmatic use) and
//! [`LocalFsLoaderComponent`] (for the Orkester plugin runtime).

mod component;
mod diff;
mod fs;
mod scanner;
mod types;
mod watcher;

#[cfg(test)]
mod tests;

// Re-export the public API so callers only need `use local_fs::...`.
pub use component::LocalFsLoaderComponent;
pub use types::{
    LocalFsChangeEvent, LocalFsEntry, LocalFsLoadedFile,
    LocalFsLoaderConfig, LocalFsLoaderEntryConfig, LocalFsScanMetrics,
};

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use chrono::Utc;
use orkester_plugin::hub::Envelope;

use workaholic::{Document, DocumentLoader, DocumentParser, Result};

use crate::document::loader::actions::*;

use scanner::scan_entry;
use watcher::spawn_entry_watcher;

// ─── LocalFsLoader ─────────────────────────────────────────────────────────────

/// Loads documents from the local filesystem.
///
/// Each [`LocalFsEntry`] represents one watched root (a file or a directory).
/// The loader maintains an in-memory cache of parsed documents so that
/// [`DocumentLoader::load`] is always cheap (no I/O on the hot path).
///
/// Call [`start`](Self::start) once after all entries are registered to perform
/// an initial scan and launch background watchers.
pub struct LocalFsLoader {
    /// Registered entries, each behind an `Arc<Mutex<...>>` so the background
    /// watcher threads can share ownership with the loader.
    pub(crate) entries:    Vec<Arc<Mutex<LocalFsEntry>>>,
    /// Extension → parser mapping, shared with all watcher threads.
    pub(crate) extensions: Arc<HashMap<String, Box<dyn DocumentParser>>>,
    /// Host handle used to fire change events.  `None` when the loader was
    /// created without a host (e.g. in unit tests).
    host_ptr: Option<orkester_plugin::sdk::HostRef>,
    /// Ring buffer of the last 200 scan results across all entries (shared with watcher threads).
    pub(crate) metrics: Arc<Mutex<VecDeque<LocalFsScanMetrics>>>,
}

impl LocalFsLoader {
    /// Creates a new loader with the given extension → parser mapping.
    /// The loader has no host connection; change events are silently dropped.
    /// Use [`new_with_host`](Self::new_with_host) in production code.
    pub fn new(extensions: HashMap<String, Box<dyn DocumentParser>>) -> Self {
        Self { entries: Vec::new(), extensions: Arc::new(extensions), host_ptr: None, metrics: Arc::new(Mutex::new(VecDeque::new())) }
    }

    /// Creates a new loader that fires change events through `host_ptr`.
    pub fn new_with_host(
        extensions: HashMap<String, Box<dyn DocumentParser>>,
        host_ptr:   *mut orkester_plugin::abi::AbiHost,
    ) -> Self {
        Self {
            entries:    Vec::new(),
            extensions: Arc::new(extensions),
            host_ptr:   Some(orkester_plugin::sdk::HostRef::new(host_ptr)),
            metrics:    Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Performs an initial scan of every registered entry, then spawns a
    /// background polling thread for each entry that has `watch = true`.
    ///
    /// Idempotent in terms of safety (calling it twice will spawn duplicate
    /// threads, so it should only be called once after entries are configured).
    /// Never panics; individual errors are logged and skipped.
    pub fn start(&mut self) {
        self.initial_scan();
        self.start_watchers();
    }

    /// Returns all documents currently held in the cache across all entries.
    pub fn load(&self) -> Result<Vec<Document>> {
        let mut documents = Vec::new();
        for entry_arc in &self.entries {
            let entry = entry_arc.lock().unwrap();
            for loaded_file in entry.loaded_files.values() {
                documents.extend(loaded_file.documents.clone());
            }
        }
        Ok(documents)
    }

    /// Returns a snapshot of recent scan metrics (up to the last 200 scans across all entries).
    pub fn recent_metrics(&self) -> Vec<LocalFsScanMetrics> {
        self.metrics.lock().unwrap().iter().cloned().collect()
    }
}

// ─── Private implementation ────────────────────────────────────────────────────

impl LocalFsLoader {
    /// Scans every entry once and emits the resulting change events.
    fn initial_scan(&mut self) {
        for entry_arc in &self.entries {
            let started = Instant::now();
            let scanned_at = Utc::now().to_rfc3339();
            let entry_path = entry_arc.lock().unwrap().path.clone();
            let events = {
                let mut entry = entry_arc.lock().unwrap();
                scan_entry(&mut entry, &self.extensions)
            };
            let duration_ms = started.elapsed().as_millis() as u64;
            let m = LocalFsScanMetrics {
                entry_path,
                scanned_at,
                is_initial:      true,
                duration_ms,
                events_added:    events.iter().filter(|e| matches!(e, LocalFsChangeEvent::DocumentAdded { .. })).count(),
                events_modified: events.iter().filter(|e| matches!(e, LocalFsChangeEvent::DocumentModified { .. })).count(),
                events_removed:  events.iter().filter(|e| matches!(e, LocalFsChangeEvent::DocumentRemoved { .. })).count(),
            };
            eprintln!(
                "[loader] initial scan '{}': {}ms, +{} ~{} -{}",
                m.entry_path, m.duration_ms, m.events_added, m.events_modified, m.events_removed,
            );
            {
                let mut store = self.metrics.lock().unwrap();
                if store.len() >= 200 { store.pop_front(); }
                store.push_back(m.clone());
            }
            for event in events {
                if let Err(e) = self.emit_change_event(event) {
                    log::error!("emit_change_event failed during initial scan: {}", e);
                }
            }
            emit_scan_metrics(self.host_ptr, &m);
        }
    }

    /// Spawns background watcher threads for every entry with `watch = true`.
    fn start_watchers(&self) {
        for entry_arc in &self.entries {
            if !entry_arc.lock().unwrap().watch {
                continue;
            }
            let loader = self.clone();
            let metrics_store = Arc::clone(&self.metrics);
            let host_opt = self.host_ptr;
            spawn_entry_watcher(
                Arc::clone(entry_arc),
                Arc::clone(&self.extensions),
                metrics_store,
                move |event| {
                    if let Err(e) = loader.emit_change_event(event) {
                        log::error!("emit_change_event failed in watcher: {}", e);
                    }
                },
                move |m| emit_scan_metrics(host_opt, m),
            );
        }
    }

    /// Fires a change event to the host using fire-and-forget mode.
    ///
    /// Wraps the event in an [`Envelope`] keyed by the appropriate action
    /// constant so the host hub can route it to interested components
    /// (e.g. the catalog server).  If no host is configured the event is
    /// silently dropped (tests, standalone use).
    fn emit_change_event(&self, event: LocalFsChangeEvent) -> Result<()> {
        let kind = match &event {
            LocalFsChangeEvent::DocumentAdded { .. }    => EVENT_LOADER_DOCUMENT_ADDED,
            LocalFsChangeEvent::DocumentRemoved { .. }  => EVENT_LOADER_DOCUMENT_REMOVED,
            LocalFsChangeEvent::DocumentModified { .. } => EVENT_LOADER_DOCUMENT_MODIFIED,
        };
        let envelope = Envelope {
            id:      0,
            kind:    kind.to_string(),
            owner:   None,
            format:  "std/json".to_string(),
            payload: serde_json::to_vec(&event)?,
        };
        if let Some(host_ref) = self.host_ptr {
            // HostRef::fire serialises as JSON+fire and returns immediately;
            // the pipeline worker routes the envelope asynchronously.
            host_ref.fire(&envelope);
        }
        Ok(())
    }
}

// ─── Metric emission helpers ───────────────────────────────────────────────────

/// Fire a single `metrics/Record` envelope through the host (fire-and-forget).
/// Does nothing when `host_ptr` is `None` (tests / standalone use).
fn fire_metric(host_ptr: Option<orkester_plugin::sdk::HostRef>, key: &str, operation: &str, value: f64) {
    let Some(host) = host_ptr else { return };
    let payload = serde_json::json!({ "key": key, "operation": operation, "value": value });
    let envelope = Envelope {
        id:      0,
        kind:    "metrics/Record".to_string(),
        owner:   None,
        format:  "std/json".to_string(),
        payload: serde_json::to_vec(&payload).unwrap_or_default(),
    };
    host.fire(&envelope);
}

/// Emit one `metrics/Record` fire-and-forget event per scan counter.
fn emit_scan_metrics(host_ptr: Option<orkester_plugin::sdk::HostRef>, m: &LocalFsScanMetrics) {
    fire_metric(host_ptr, "workaholic.local_fs_loader.scans",              "increase", 1.0);
    fire_metric(host_ptr, "workaholic.local_fs_loader.documents.added",    "increase", m.events_added    as f64);
    fire_metric(host_ptr, "workaholic.local_fs_loader.documents.modified", "increase", m.events_modified as f64);
    fire_metric(host_ptr, "workaholic.local_fs_loader.documents.removed",  "increase", m.events_removed  as f64);
    fire_metric(host_ptr, "workaholic.local_fs_loader.scan_duration_ms",   "set",      m.duration_ms     as f64);
}

// ─── Trait implementations ─────────────────────────────────────────────────────

impl Clone for LocalFsLoader {
    fn clone(&self) -> Self {
        Self {
            entries:    self.entries.iter().map(Arc::clone).collect(),
            extensions: Arc::clone(&self.extensions),
            host_ptr:   self.host_ptr,  // Copy — HostRef is Copy
            metrics:    Arc::clone(&self.metrics),
        }
    }
}

impl DocumentLoader for LocalFsLoader {
    fn load(&self) -> Result<Vec<Document>> {
        LocalFsLoader::load(self)
    }
}
