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

use workaholic::{Document, DocumentLoader, DocumentParser, Result};

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
}

impl LocalFsLoader {
    /// Creates a new loader with the given extension → parser mapping.
    pub fn new(extensions: HashMap<String, Box<dyn DocumentParser>>) -> Self {
        Self { entries: Vec::new(), extensions: Arc::new(extensions) }
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

    /// Forwards a change event to the configured subscriber(s).
    ///
    /// Not yet implemented — the body is intentionally left empty until an
    /// event-bus integration is wired in.
    fn emit_change_event(&self, _event: LocalFsChangeEvent) -> Result<()> {
        // TODO: forward to an event bus / subscriber list.
        Ok(())
    }
}

// ─── Trait implementations ─────────────────────────────────────────────────────

impl Clone for LocalFsLoader {
    fn clone(&self) -> Self {
        Self {
            entries:    self.entries.iter().map(Arc::clone).collect(),
            extensions: Arc::clone(&self.extensions),
        }
    }
}

impl DocumentLoader for LocalFsLoader {
    fn load(&self) -> Result<Vec<Document>> {
        LocalFsLoader::load(self)
    }
}
