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
    LocalFsLoaderConfig, LocalFsLoaderEntryConfig,
};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
}

impl LocalFsLoader {
    /// Creates a new loader with the given extension → parser mapping.
    /// The loader has no host connection; change events are silently dropped.
    /// Use [`new_with_host`](Self::new_with_host) in production code.
    pub fn new(extensions: HashMap<String, Box<dyn DocumentParser>>) -> Self {
        Self { entries: Vec::new(), extensions: Arc::new(extensions), host_ptr: None }
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
}

// ─── Private implementation ────────────────────────────────────────────────────

impl LocalFsLoader {
    /// Scans every entry once and emits the resulting change events.
    fn initial_scan(&mut self) {
        for entry_arc in &self.entries {
            let events = {
                let mut entry = entry_arc.lock().unwrap();
                scan_entry(&mut entry, &self.extensions)
            };
            for event in events {
                if let Err(e) = self.emit_change_event(event) {
                    log::error!("emit_change_event failed during initial scan: {}", e);
                }
            }
        }
    }

    /// Spawns background watcher threads for every entry with `watch = true`.
    fn start_watchers(&self) {
        for entry_arc in &self.entries {
            if !entry_arc.lock().unwrap().watch {
                continue;
            }
            let loader = self.clone();
            spawn_entry_watcher(
                Arc::clone(entry_arc),
                Arc::clone(&self.extensions),
                move |event| {
                    if let Err(e) = loader.emit_change_event(event) {
                        log::error!("emit_change_event failed in watcher: {}", e);
                    }
                },
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

// ─── Trait implementations ─────────────────────────────────────────────────────

impl Clone for LocalFsLoader {
    fn clone(&self) -> Self {
        Self {
            entries:    self.entries.iter().map(Arc::clone).collect(),
            extensions: Arc::clone(&self.extensions),
            host_ptr:   self.host_ptr,  // Copy — HostRef is Copy
        }
    }
}

impl DocumentLoader for LocalFsLoader {
    fn load(&self) -> Result<Vec<Document>> {
        LocalFsLoader::load(self)
    }
}
