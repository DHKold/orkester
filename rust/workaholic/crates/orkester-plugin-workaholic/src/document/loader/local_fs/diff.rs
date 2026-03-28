//! Document-level diffing: identity keys, equality checks, and event generation.
//!
//! Diffing happens at two granularities:
//!  - **File level** — file added / deleted → all documents in it are added / removed.
//!  - **Document level** — file modified → individual documents are compared to find
//!    exactly which ones were added, removed, or changed.

use std::collections::HashMap;

use workaholic::Document;

use super::types::{LocalFsChangeEvent, LocalFsLoadedFile};

// ─── Document identity ─────────────────────────────────────────────────────────

/// Returns the stable identity key of a document: `"{kind}/{name}"`.
///
/// Used to match documents across different versions of the same file.
pub fn doc_key(doc: &Document) -> String {
    format!("{}/{}", doc.kind, doc.name)
}

/// Returns `true` when `a` and `b` are semantically identical.
///
/// Two documents are considered equal when their `version`, `spec`, and `metadata`
/// all serialise to the same JSON. The `status` field is intentionally ignored
/// because it is runtime state, not source-of-truth content.
pub fn docs_equal(a: &Document, b: &Document) -> bool {
    a.version == b.version
        && serde_json::to_string(&a.spec).ok() == serde_json::to_string(&b.spec).ok()
        && serde_json::to_string(&a.metadata).ok() == serde_json::to_string(&b.metadata).ok()
}

// ─── File-level event generation ──────────────────────────────────────────────

/// Builds a `DocumentRemoved` event for each document in a file that disappeared from disk.
pub fn events_for_deleted_file(
    loaded_file: &LocalFsLoadedFile,
    entry_path: &str,
) -> Vec<LocalFsChangeEvent> {
    loaded_file
        .documents
        .iter()
        .map(|doc| LocalFsChangeEvent::DocumentRemoved {
            entry_path:  entry_path.to_string(),
            source_path: loaded_file.path.clone(),
            document:    doc.clone(),
        })
        .collect()
}

/// Builds a `DocumentAdded` event for each document in a brand-new file.
pub fn events_for_new_file(
    docs: &[Document],
    entry_path: &str,
    source_path: &str,
) -> Vec<LocalFsChangeEvent> {
    docs.iter()
        .map(|doc| LocalFsChangeEvent::DocumentAdded {
            entry_path:  entry_path.to_string(),
            source_path: source_path.to_string(),
            document:    doc.clone(),
        })
        .collect()
}

// ─── Document-level diff for modified files ────────────────────────────────────

/// Yields `DocumentRemoved` and `DocumentModified` events for documents that
/// disappeared or changed between `old_map` and `new_map`.
fn removed_and_modified_events<'a>(
    old_map: &HashMap<String, &'a Document>,
    new_map: &HashMap<String, &'a Document>,
    entry_path: &str,
    source_path: &str,
) -> Vec<LocalFsChangeEvent> {
    let mut events = Vec::new();
    for (key, old_doc) in old_map {
        match new_map.get(key.as_str()) {
            Some(new_doc) if !docs_equal(old_doc, new_doc) => {
                events.push(LocalFsChangeEvent::DocumentModified {
                    entry_path:  entry_path.to_string(),
                    source_path: source_path.to_string(),
                    old: (*old_doc).clone(),
                    new: (*new_doc).clone(),
                });
            }
            None => {
                events.push(LocalFsChangeEvent::DocumentRemoved {
                    entry_path:  entry_path.to_string(),
                    source_path: source_path.to_string(),
                    document:    (*old_doc).clone(),
                });
            }
            _ => {} // present and unchanged
        }
    }
    events
}

/// Yields `DocumentAdded` events for documents that appear only in `new_map`.
fn added_events<'a>(
    old_map: &HashMap<String, &'a Document>,
    new_map: &HashMap<String, &'a Document>,
    entry_path: &str,
    source_path: &str,
) -> Vec<LocalFsChangeEvent> {
    new_map
        .iter()
        .filter(|(key, _)| !old_map.contains_key(key.as_str()))
        .map(|(_, doc)| LocalFsChangeEvent::DocumentAdded {
            entry_path:  entry_path.to_string(),
            source_path: source_path.to_string(),
            document:    (*doc).clone(),
        })
        .collect()
}

/// Diffs `old_docs` against `new_docs` from a **modified** file and returns the
/// resulting change events at document granularity.
///
/// Produces `DocumentAdded`, `DocumentModified`, and `DocumentRemoved` as needed.
pub fn diff_modified_file(
    old_docs: &[Document],
    new_docs: &[Document],
    entry_path: &str,
    source_path: &str,
) -> Vec<LocalFsChangeEvent> {
    let old_map: HashMap<String, &Document> =
        old_docs.iter().map(|d| (doc_key(d), d)).collect();
    let new_map: HashMap<String, &Document> =
        new_docs.iter().map(|d| (doc_key(d), d)).collect();

    let mut events = removed_and_modified_events(&old_map, &new_map, entry_path, source_path);
    events.extend(added_events(&old_map, &new_map, entry_path, source_path));
    events
}
