//! Orkester component wrapper for [`S3Loader`].

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use orkester_plugin::prelude::*;
use workaholic::{Document, DocumentParser, WorkaholicError};

use crate::document::loader::actions::*;
use crate::document::parser::json::JsonDocumentParser;
use crate::document::parser::yaml::YamlDocumentParser;

use super::types::{S3Entry, S3LoaderConfig, S3LoaderEntryConfig, S3ScanMetrics};
use super::loader::S3Loader;

pub struct S3LoaderComponent { loader: S3Loader }

fn build_parsers(config: &S3LoaderConfig) -> HashMap<String, Box<dyn DocumentParser>> {
    let mut p: HashMap<String, Box<dyn DocumentParser>> = HashMap::new();
    for (ext, kind) in &config.extensions {
        match kind.as_str() {
            "workaholic/YamlDocumentParser:1.0" | "yaml" | "yml" => { p.insert(ext.clone(), Box::new(YamlDocumentParser)); }
            "workaholic/JsonDocumentParser:1.0" | "json"         => { p.insert(ext.clone(), Box::new(JsonDocumentParser)); }
            other => { log::warn!("Unknown parser kind '{other}' for extension '{ext}' — skipped"); }
        }
    }
    p
}

fn find_entry_idx(entries: &[Arc<Mutex<S3Entry>>], id: &str) -> workaholic::Result<usize> {
    entries.iter().position(|a| { let e = a.lock().unwrap(); format!("s3://{}/{}", e.config.bucket, e.config.prefix) == id })
        .ok_or_else(|| WorkaholicError::NotFound { kind: "S3Entry".into(), name: id.into() })
}

#[component(
    kind        = "workaholic/S3Loader:1.0",
    name        = "S3 Document Loader",
    description = "Loader that reads documents from S3-compatible object storage (AWS S3, MinIO, etc.).",
)]
impl S3LoaderComponent {
    pub fn new(host_ptr: *mut orkester_plugin::abi::AbiHost, config: S3LoaderConfig) -> Self {
        Self { loader: S3Loader::new_with_host(build_parsers(&config), host_ptr) }
    }

    #[handle(ACTION_LOAD_DOCUMENTS)]
    fn handle_load(&mut self, _: String) -> workaholic::Result<Vec<Document>> { self.loader.load() }

    #[handle(ACTION_LOADER_START)]
    fn handle_start(&mut self, _: serde_json::Value) -> workaholic::Result<()> { self.loader.start(); Ok(()) }

    #[handle(ACTION_LOADER_CREATE_ENTRY)]
    fn handle_add_entry(&mut self, cfg: S3LoaderEntryConfig) -> workaholic::Result<String> {
        Ok(self.loader.add_entry(cfg))
    }

    #[handle(ACTION_LOADER_RETRIEVE_ENTRY)]
    fn handle_retrieve_entry(&mut self, entry_id: String) -> workaholic::Result<S3LoaderEntryConfig> {
        let idx = find_entry_idx(&self.loader.entries, &entry_id)?;
        Ok(self.loader.entries[idx].lock().unwrap().config.clone())
    }

    #[handle(ACTION_LOADER_UPDATE_ENTRY)]
    fn handle_update_entry(&mut self, (entry_id, cfg): (String, S3LoaderEntryConfig)) -> workaholic::Result<()> {
        let idx = find_entry_idx(&self.loader.entries, &entry_id)?;
        self.loader.entries[idx].lock().unwrap().config = cfg;
        Ok(())
    }

    #[handle(ACTION_LOADER_DELETE_ENTRY)]
    fn handle_delete_entry(&mut self, entry_id: String) -> workaholic::Result<()> {
        let idx = find_entry_idx(&self.loader.entries, &entry_id)?;
        self.loader.entries.remove(idx);
        Ok(())
    }

    #[handle(ACTION_LOADER_SEARCH_ENTRIES)]
    fn handle_search_entries(&mut self, _: String) -> workaholic::Result<HashMap<String, S3LoaderEntryConfig>> {
        Ok(self.loader.entries.iter().map(|a| { let e = a.lock().unwrap(); (format!("s3://{}/{}", e.config.bucket, e.config.prefix), e.config.clone()) }).collect())
    }

    #[handle(ACTION_LOADER_GET_METRICS)]
    fn handle_metrics(&mut self, _: String) -> workaholic::Result<Vec<S3ScanMetrics>> {
        Ok(self.loader.recent_metrics())
    }
}
