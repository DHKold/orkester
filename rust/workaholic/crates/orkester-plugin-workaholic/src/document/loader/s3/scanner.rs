//! S3 entry scanner: compares live bucket state against cache to produce events.

use std::collections::HashMap;

use workaholic::DocumentParser;

use super::client::{get_object, list_objects};
use super::types::{S3ChangeEvent, S3Entry, S3LoadedObject};

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn entry_id(entry: &S3Entry) -> String {
    format!("s3://{}/{}", entry.config.bucket, entry.config.prefix)
}

fn parse_bytes(bytes: &[u8], key: &str, ext_parsers: &HashMap<String, Box<dyn DocumentParser>>) -> Vec<workaholic::Document> {
    let ext = key.rsplit('.').next().unwrap_or("").to_lowercase();
    if let Some(parser) = ext_parsers.get(&ext) {
        let content = std::str::from_utf8(bytes).unwrap_or("");
        parser.parse(content).unwrap_or_default()
    } else {
        vec![]
    }
}

fn events_for_new(entry: &S3Entry, key: &str, docs: &[workaholic::Document]) -> Vec<S3ChangeEvent> {
    let id = entry_id(entry);
    docs.iter().map(|d| S3ChangeEvent::DocumentAdded { entry_id: id.clone(), key: key.to_string(), document: d.clone() }).collect()
}

fn events_for_modified(entry: &S3Entry, key: &str, old_docs: &[workaholic::Document], new_docs: &[workaholic::Document]) -> Vec<S3ChangeEvent> {
    let id = entry_id(entry);
    old_docs.iter().zip(new_docs).map(|(o, n)| S3ChangeEvent::DocumentModified { entry_id: id.clone(), key: key.to_string(), old: o.clone(), new: n.clone() }).collect()
}

fn events_for_deleted(entry: &S3Entry, loaded: &S3LoadedObject) -> Vec<S3ChangeEvent> {
    let id = entry_id(entry);
    loaded.documents.iter().map(|d| S3ChangeEvent::DocumentRemoved { entry_id: id.clone(), key: loaded.key.clone(), document: d.clone() }).collect()
}

// ─── Scanner ──────────────────────────────────────────────────────────────────

/// Scan the S3 entry and return change events (new, modified, deleted objects).
pub fn scan_entry(entry: &mut S3Entry, ext_parsers: &HashMap<String, Box<dyn DocumentParser>>) -> Vec<S3ChangeEvent> {
    let objects = match list_objects(&entry.config) {
        Ok(v)  => v,
        Err(e) => { log::error!("[s3] list_objects failed: {e}"); return vec![]; }
    };

    let mut events        = Vec::new();
    let mut current_keys: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (key, etag) in objects {
        current_keys.insert(key.clone());
        let cached_etag = entry.loaded_objects.get(&key).map(|o| o.etag.clone());

        if cached_etag.as_deref() == Some(&etag) { continue; } // unchanged

        let bytes = match get_object(&entry.config, &key) {
            Ok(b)  => b,
            Err(e) => { log::warn!("[s3] get_object({key}) failed: {e}"); continue; }
        };
        let docs = parse_bytes(&bytes, &key, ext_parsers);
        if let Some(old) = entry.loaded_objects.get(&key) {
            events.extend(events_for_modified(entry, &key, &old.documents.clone(), &docs));
        } else {
            events.extend(events_for_new(entry, &key, &docs));
        }
        entry.loaded_objects.insert(key.clone(), S3LoadedObject { key, etag, documents: docs });
    }

    // Detect removals
    let removed: Vec<_> = entry.loaded_objects.keys().filter(|k| !current_keys.contains(*k)).cloned().collect();
    for key in removed {
        if let Some(loaded) = entry.loaded_objects.remove(&key) {
            events.extend(events_for_deleted(entry, &loaded));
        }
    }
    events
}
