//! Raw filesystem utilities: file collection and parsing.
//!
//! All functions are resilient — I/O errors are logged and silently skipped so
//! callers never need to handle infrastructure failures themselves.

use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

use workaholic::{Document, DocumentParser, Result, WorkaholicError};
use orkester_plugin::{log_trace, log_warn};

// ─── Extension helpers ─────────────────────────────────────────────────────────

/// Returns the lowercase file extension of `path`, or `None` if absent or non-UTF-8.
pub fn get_extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
}

/// Returns the `mtime` from `metadata`, logging a warning and returning `None` on failure.
pub fn get_mtime(path: &str, meta: &std::fs::Metadata) -> Option<SystemTime> {
    meta.modified()
        .map_err(|e| log_warn!("Cannot read mtime for '{}': {}", path, e))
        .ok()
}

// ─── File collection ───────────────────────────────────────────────────────────

/// Inserts `path` into `files` if it carries a supported extension and a readable `mtime`.
fn try_insert_file(
    path: &str,
    meta: &std::fs::Metadata,
    extensions: &HashMap<String, Box<dyn DocumentParser>>,
    files: &mut HashMap<String, SystemTime>,
) {
    if let Some(ext) = get_extension(path) {
        if extensions.contains_key(&ext) {
            if let Some(mtime) = get_mtime(path, meta) {
                files.insert(path.to_string(), mtime);
            }
        }
    }
}

/// Reads one directory level and forwards each child to [`collect_files_inner`].
fn collect_dir_entries(
    dir_path: &str,
    recursive: bool,
    extensions: &HashMap<String, Box<dyn DocumentParser>>,
    files: &mut HashMap<String, SystemTime>,
) {
    let read_dir = match std::fs::read_dir(dir_path) {
        Ok(it) => it,
        Err(e) => { log_warn!("Cannot list '{}': {}", dir_path, e); return; }
    };
    for entry in read_dir {
        let entry = match entry {
            Ok(e)  => e,
            Err(e) => { log_warn!("Error reading dir entry: {}", e); continue; }
        };
        let child = match entry.path().to_str().map(str::to_string) {
            Some(s) => s,
            None    => { log_warn!("Non-UTF-8 path skipped"); continue; }
        };
        let child_meta = match std::fs::metadata(&child) {
            Ok(m)  => m,
            Err(e) => { log_warn!("Cannot stat '{}': {}", child, e); continue; }
        };
        if child_meta.is_file() {
            try_insert_file(&child, &child_meta, extensions, files);
        } else if child_meta.is_dir() && recursive {
            log_trace!("[local_fs/fs] recursing into '{}'", child);
            collect_files_inner(&child, recursive, extensions, files);
        }
    }
}

/// Dispatches between file and directory, recursing into directories when enabled.
fn collect_files_inner(
    path: &str,
    recursive: bool,
    extensions: &HashMap<String, Box<dyn DocumentParser>>,
    files: &mut HashMap<String, SystemTime>,
) {
    let meta = match std::fs::metadata(path) {
        Ok(m)  => m,
        Err(e) => { log_warn!("Cannot stat '{}': {}", path, e); return; }
    };
    if meta.is_file() {
        try_insert_file(path, &meta, extensions, files);
    } else if meta.is_dir() {
        collect_dir_entries(path, recursive, extensions, files);
    }
}

/// Returns a `path → mtime` map for every file with a supported extension reachable
/// from `root`, respecting the `recursive` flag.
///
/// Unreadable paths are logged and skipped — the function never fails.
pub fn collect_files(
    root: &str,
    recursive: bool,
    extensions: &HashMap<String, Box<dyn DocumentParser>>,
) -> HashMap<String, SystemTime> {
    let mut files = HashMap::new();
    collect_files_inner(root, recursive, extensions, &mut files);
    files
}

// ─── Parsing ──────────────────────────────────────────────────────────────────

/// Reads and parses `file_path`, returning every document it contains.
///
/// Returns an `Err` if the extension is unsupported, the file cannot be read,
/// or the parser rejects the content. Callers should log the error and skip
/// the file rather than propagating or panicking.
pub fn parse_file(
    file_path: &str,
    extensions: &HashMap<String, Box<dyn DocumentParser>>,
) -> Result<Vec<Document>> {
    let ext = get_extension(file_path).ok_or_else(|| {
        WorkaholicError::InvalidDocument(format!("'{}' has no file extension", file_path))
    })?;

    let parser = extensions.get(&ext).ok_or_else(|| {
        WorkaholicError::InvalidDocument(
            format!("No parser registered for extension '{}' (file: '{}')", ext, file_path),
        )
    })?;

    let content = std::fs::read_to_string(file_path)
        .map_err(|e| WorkaholicError::Other(format!("Cannot read '{}': {}", file_path, e)))?;

    parser.parse(&content)
}
