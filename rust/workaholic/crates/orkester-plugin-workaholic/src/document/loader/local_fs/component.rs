//! Orkester component wrapper for [`LocalFsLoader`].
//!
//! Translates the generic Orkester action protocol into typed calls on the loader.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use orkester_plugin::prelude::*;
use workaholic::{Document, DocumentParser, WorkaholicError};

use crate::document::loader::actions::*;
use crate::document::parser::json::JsonDocumentParser;
use crate::document::parser::yaml::YamlDocumentParser;

use super::{LocalFsLoader, LocalFsEntry, LocalFsLoaderConfig, LocalFsLoaderEntryConfig, LocalFsScanMetrics};

// ─── Component struct ─────────────────────────────────────────────────────────

/// Orkester component that exposes [`LocalFsLoader`] through the standard action protocol.
pub struct LocalFsLoaderComponent {
    loader: LocalFsLoader,
}

// ─── Private helpers ──────────────────────────────────────────────────────────

/// Converts the `extensions` map from the config into a live parser map.
fn build_parsers(config: &LocalFsLoaderConfig) -> HashMap<String, Box<dyn DocumentParser>> {
    let mut parsers: HashMap<String, Box<dyn DocumentParser>> = HashMap::new();
    for (ext, kind) in &config.extensions {
        match kind.as_str() {
            "workaholic/YamlDocumentParser:1.0" | "yaml" | "yml" => {
                parsers.insert(ext.clone(), Box::new(YamlDocumentParser));
            }
            "workaholic/JsonDocumentParser:1.0" | "json" => {
                parsers.insert(ext.clone(), Box::new(JsonDocumentParser));
            }
            other => {
                log_warn!("Unknown parser kind '{}' for extension '{}' — skipped", other, ext);
            }
        }
    }
    parsers
}

/// Converts a locked [`LocalFsEntry`] into its serialisable configuration DTO.
fn entry_to_config(entry: &LocalFsEntry) -> LocalFsLoaderEntryConfig {
    LocalFsLoaderEntryConfig {
        path:      entry.path.clone(),
        recursive: entry.recursive,
        watch:     entry.watch,
    }
}

/// Finds the entry whose `path` equals `entry_id`, or returns a `NotFound` error.
fn find_entry_arc<'a>(
    entries: &'a [Arc<Mutex<LocalFsEntry>>],
    entry_id: &str,
) -> workaholic::Result<&'a Arc<Mutex<LocalFsEntry>>> {
    entries
        .iter()
        .find(|arc| arc.lock().unwrap().path == entry_id)
        .ok_or_else(|| WorkaholicError::NotFound {
            kind: "LocalFsEntry".into(),
            name: entry_id.into(),
        })
}

// ─── Component implementation ─────────────────────────────────────────────────

#[component(
    kind        = "workaholic/LocalFsLoader:1.0",
    name        = "Local Filesystem Loader",
    description = "Loader that reads documents from the local filesystem based on specified paths and parameters.",
)]
impl LocalFsLoaderComponent {
    pub fn new(host_ptr: *mut orkester_plugin::abi::AbiHost, config: LocalFsLoaderConfig) -> Self {
        Self { loader: LocalFsLoader::new_with_host(build_parsers(&config), host_ptr) }
    }

    /// Returns all documents currently held in the cache across all entries.
    #[handle(ACTION_LOAD_DOCUMENTS)]
    fn handle_load(&mut self, _: String) -> workaholic::Result<Vec<Document>> {
        self.loader.load()
    }

    /// Starts the loader: performs an initial scan of every registered entry, then
    #[handle(ACTION_LOADER_START)]
    fn handle_start_loader(&mut self, _: serde_json::Value) -> workaholic::Result<()> {
          self.loader.start();
        Ok(())
    }

    /// Creates a new watched entry and returns its generated ID.
    #[handle(ACTION_LOADER_CREATE_ENTRY)]
    fn handle_add_entry(
        &mut self,
        entry_config: LocalFsLoaderEntryConfig,
    ) -> workaholic::Result<String> {
        let entry_id = format!("entry-{}", self.loader.entries.len() + 1);
        self.loader.entries.push(Arc::new(Mutex::new(LocalFsEntry {
            path:         entry_config.path,
            recursive:    entry_config.recursive,
            watch:        entry_config.watch,
            loaded_files: HashMap::new(),
        })));
        Ok(entry_id)
    }

    /// Returns the configuration of the entry identified by `entry_id` (its path).
    #[handle(ACTION_LOADER_RETRIEVE_ENTRY)]
    fn handle_retrieve_entry(&mut self, entry_id: String) -> workaholic::Result<LocalFsLoaderEntryConfig> {
        let arc = find_entry_arc(&self.loader.entries, &entry_id)?;
        Ok(entry_to_config(&arc.lock().unwrap()))
    }

    /// Overwrites the configuration of an existing entry.
    #[handle(ACTION_LOADER_UPDATE_ENTRY)]
    fn handle_update_entry(
        &mut self,
        (entry_id, config): (String, LocalFsLoaderEntryConfig),
    ) -> workaholic::Result<()> {
        let arc = find_entry_arc(&self.loader.entries, &entry_id)?;
        let mut entry  = arc.lock().unwrap();
        entry.path      = config.path;
        entry.recursive = config.recursive;
        entry.watch     = config.watch;
        Ok(())
    }

    /// Removes the entry identified by `entry_id` from the loader.
    #[handle(ACTION_LOADER_DELETE_ENTRY)]
    fn handle_delete_entry(&mut self, entry_id: String) -> workaholic::Result<()> {
        let index = self.loader.entries
            .iter()
            .position(|arc| arc.lock().unwrap().path == entry_id)
            .ok_or_else(|| WorkaholicError::NotFound {
                kind: "LocalFsEntry".into(),
                name: entry_id.clone(),
            })?;
        self.loader.entries.remove(index);
        Ok(())
    }

    /// Returns all entries, keyed by their path.
    #[handle(ACTION_LOADER_SEARCH_ENTRIES)]
    fn handle_search_entries(
        &mut self,
        _config: String,
    ) -> workaholic::Result<HashMap<String, LocalFsLoaderEntryConfig>> {
        Ok(self.loader.entries
            .iter()
            .map(|arc| {
                let entry = arc.lock().unwrap();
                (entry.path.clone(), entry_to_config(&entry))
            })
            .collect())
    }

    /// Returns recent scan metrics (timing, event counts) for all watched entries.
    #[handle(ACTION_LOADER_GET_METRICS)]
    fn handle_get_metrics(&mut self, _: serde_json::Value) -> workaholic::Result<Vec<LocalFsScanMetrics>> {
        Ok(self.loader.recent_metrics())
    }
}
