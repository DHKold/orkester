//! [`S3Loader`]: programmatic interface for loading documents from S3/MinIO.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use orkester_plugin::hub::Envelope;
use workaholic::{Document, DocumentParser, Result};

use crate::document::loader::actions::*;
use super::scanner::scan_entry;
use super::types::{S3ChangeEvent, S3Entry, S3ScanMetrics};
use super::types::{S3LoaderEntryConfig};
use super::watcher::{build_metrics_pub, spawn_entry_watcher};

pub struct S3Loader {
    pub(crate) entries:    Vec<Arc<Mutex<S3Entry>>>,
    pub(crate) extensions: Arc<HashMap<String, Box<dyn DocumentParser>>>,
    host_ptr: Option<orkester_plugin::sdk::HostRef>,
    pub(crate) metrics: Arc<Mutex<VecDeque<S3ScanMetrics>>>,
}

impl S3Loader {
    pub fn new(extensions: HashMap<String, Box<dyn DocumentParser>>) -> Self {
        Self { entries: Vec::new(), extensions: Arc::new(extensions), host_ptr: None, metrics: Arc::new(Mutex::new(VecDeque::new())) }
    }

    pub fn new_with_host(extensions: HashMap<String, Box<dyn DocumentParser>>, host_ptr: *mut orkester_plugin::abi::AbiHost) -> Self {
        Self { entries: Vec::new(), extensions: Arc::new(extensions), host_ptr: Some(orkester_plugin::sdk::HostRef::new(host_ptr)), metrics: Arc::new(Mutex::new(VecDeque::new())) }
    }

    pub fn add_entry(&mut self, cfg: S3LoaderEntryConfig) -> String {
        let id = format!("s3://{}/{}", cfg.bucket, cfg.prefix);
        self.entries.push(Arc::new(Mutex::new(S3Entry { config: cfg, loaded_objects: HashMap::new() })));
        id
    }

    pub fn load(&self) -> Result<Vec<Document>> {
        let mut docs = Vec::new();
        for arc in &self.entries {
            let entry = arc.lock().unwrap();
            for obj in entry.loaded_objects.values() { docs.extend(obj.documents.clone()); }
        }
        Ok(docs)
    }

    pub fn recent_metrics(&self) -> Vec<S3ScanMetrics> { self.metrics.lock().unwrap().iter().cloned().collect() }

    pub fn start(&mut self) { self.initial_scan(); self.start_watchers(); }
}

impl S3Loader {
    fn initial_scan(&mut self) {
        for arc in &self.entries {
            let started = Instant::now();
            let entry_id = arc.lock().map(|e| format!("s3://{}/{}", e.config.bucket, e.config.prefix)).unwrap_or_default();
            let events = { let mut e = arc.lock().unwrap(); scan_entry(&mut e, &self.extensions) };
            let m = build_metrics_pub(entry_id, true, started, &events);
            eprintln!("[s3] initial scan '{}': {}ms +{} ~{} -{}", m.entry_id, m.duration_ms, m.events_added, m.events_modified, m.events_removed);
            { let mut s = self.metrics.lock().unwrap(); if s.len() >= 200 { s.pop_front(); } s.push_back(m); }
            for ev in events { if let Err(e) = self.emit(ev) { log::error!("emit: {e}"); } }
        }
    }

    fn start_watchers(&self) {
        for arc in &self.entries {
            let poll_secs = arc.lock().map(|e| e.config.poll_interval_secs).unwrap_or(30);
            if !arc.lock().map(|e| e.config.watch).unwrap_or(false) { continue; }
            let loader = self.clone();
            spawn_entry_watcher(Arc::clone(arc), Arc::clone(&self.extensions), Arc::clone(&self.metrics), poll_secs, move |ev| { if let Err(e) = loader.emit(ev) { log::error!("emit: {e}"); } });
        }
    }

    fn emit(&self, event: S3ChangeEvent) -> Result<()> {
        let kind = match &event { S3ChangeEvent::DocumentAdded { .. } => EVENT_LOADER_DOCUMENT_ADDED, S3ChangeEvent::DocumentModified { .. } => EVENT_LOADER_DOCUMENT_MODIFIED, S3ChangeEvent::DocumentRemoved { .. } => EVENT_LOADER_DOCUMENT_REMOVED };
        let Some(host) = self.host_ptr else { return Ok(()); };
        let envelope = Envelope { id: 0, kind: kind.to_string(), owner: None, format: "std/json".to_string(), payload: serde_json::to_vec(&event)? };
        host.fire(&envelope);
        Ok(())
    }
}

impl Clone for S3Loader {
    fn clone(&self) -> Self {
        Self { entries: self.entries.clone(), extensions: Arc::clone(&self.extensions), host_ptr: self.host_ptr.clone(), metrics: Arc::clone(&self.metrics) }
    }
}
