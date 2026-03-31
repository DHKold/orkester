//! Entry scanning: orchestrates filesystem collection and document diffing to
//! produce change events and keep the in-memory cache up to date.

use std::collections::HashMap;
use std::time::SystemTime;

use workaholic::{Document, DocumentParser};
use orkester_plugin::{log_error, log_trace};

use super::diff::{diff_modified_file, events_for_deleted_file, events_for_new_file};
use super::fs::{collect_files, parse_file};
use super::types::{LocalFsChangeEvent, LocalFsEntry, LocalFsLoadedFile};

// ─── Deletion processing ───────────────────────────────────────────────────────

/// Emits `DocumentRemoved` events for every file in the cache that is no longer
/// present on disk, then prunes those entries from the cache.
fn process_deleted_files(
    entry: &mut LocalFsEntry,
    current_files: &HashMap<String, SystemTime>,
) -> Vec<LocalFsChangeEvent> {
    let mut events = Vec::new();
    for (path, loaded_file) in &entry.loaded_files {
        if !current_files.contains_key(path) {
            events.extend(events_for_deleted_file(loaded_file, &entry.path));
        }
    }
    entry.loaded_files.retain(|path, _| current_files.contains_key(path));
    events
}

// ─── Addition / modification processing ───────────────────────────────────────

/// Overwrites the in-memory cache entry for `file_path` with the freshly parsed data.
fn update_cache(
    entry: &mut LocalFsEntry,
    file_path: &str,
    mtime: SystemTime,
    docs: Vec<Document>,
) {
    entry.loaded_files.insert(
        file_path.to_string(),
        LocalFsLoadedFile {
            path:        file_path.to_string(),
            last_parsed: mtime,
            documents:   docs,
        },
    );
}

/// Processes a single file that is either new or has a newer `mtime` than the cache.
///
/// - New file → `DocumentAdded` events for every document in it.
/// - Modified file → document-level diff against the cached version.
/// - Unchanged file → no events, no work.
/// - Parse error → logged and skipped; `Vec::new()` is returned.
fn process_single_file(
    entry: &mut LocalFsEntry,
    file_path: &str,
    mtime: SystemTime,
    extensions: &HashMap<String, Box<dyn DocumentParser>>,
) -> Vec<LocalFsChangeEvent> {
    let is_new      = !entry.loaded_files.contains_key(file_path);
    let is_modified = !is_new && entry.loaded_files[file_path].last_parsed < mtime;

    if !is_new && !is_modified {
        return Vec::new();
    }

    match parse_file(file_path, extensions) {
        Err(e) => {
            log_error!("Skipping '{}': {}", file_path, e);
            Vec::new()
        }
        Ok(new_docs) => {
            log_trace!("[scanner] parsed '{}': {} doc(s)", file_path, new_docs.len());
            let events = if is_new {
                events_for_new_file(&new_docs, &entry.path, file_path)
            } else {
                // Clone old docs before mutably updating the cache below.
                let old_docs = entry.loaded_files[file_path].documents.clone();
                diff_modified_file(&old_docs, &new_docs, &entry.path, file_path)
            };
            update_cache(entry, file_path, mtime, new_docs);
            events
        }
    }
}

/// Iterates over all currently on-disk files and processes each addition or modification.
fn process_new_and_modified_files(
    entry: &mut LocalFsEntry,
    current_files: &HashMap<String, SystemTime>,
    extensions: &HashMap<String, Box<dyn DocumentParser>>,
) -> Vec<LocalFsChangeEvent> {
    // Collect owned keys+values first to avoid holding a borrow on `current_files`
    // while mutably borrowing `entry` inside `process_single_file`.
    let snapshot: Vec<(String, SystemTime)> =
        current_files.iter().map(|(k, v)| (k.clone(), *v)).collect();

    snapshot
        .into_iter()
        .flat_map(|(file_path, mtime)| process_single_file(entry, &file_path, mtime, extensions))
        .collect()
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Scans the filesystem paths for `entry`, diffs against the in-memory cache,
/// updates the cache in-place, and returns all change events to emit.
///
/// Never panics. All I/O errors and parse failures are logged and skipped so the
/// caller remains alive regardless of individual file problems.
pub fn scan_entry(
    entry: &mut LocalFsEntry,
    extensions: &HashMap<String, Box<dyn DocumentParser>>,
) -> Vec<LocalFsChangeEvent> {
    let current_files = collect_files(&entry.path, entry.recursive, extensions);
    let mut events    = process_deleted_files(entry, &current_files);
    events.extend(process_new_and_modified_files(entry, &current_files, extensions));
    events
}
