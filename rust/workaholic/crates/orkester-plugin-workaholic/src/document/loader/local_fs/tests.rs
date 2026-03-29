//! Unit tests for the `local_fs` module.
//!
//! Tests are grouped by the functional requirement they verify, not by the
//! module they happen to call. This file is compiled only in `--test` mode.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use serde_json::json;
use workaholic::{Document, DocumentMetadata, DocumentParser};

use super::diff::{diff_modified_file, doc_key, docs_equal, events_for_deleted_file, events_for_new_file};
use super::fs::{collect_files, get_extension, parse_file};
use super::scanner::scan_entry;
use super::types::{LocalFsChangeEvent, LocalFsEntry, LocalFsLoadedFile};
use super::LocalFsLoader;
use crate::document::parser::yaml::YamlDocumentParser;

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// A temporary directory that is removed (best-effort) when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let dir = std::env::temp_dir()
            .join(format!("orkester_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn path(&self) -> &std::path::Path { &self.0 }

    fn child(&self, name: &str) -> PathBuf { self.0.join(name) }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let p = self.child(name);
        std::fs::write(&p, content).unwrap();
        p
    }
}

impl Drop for TempDir {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

/// Creates a minimal [`Document`] with the given identity fields.
fn make_doc(kind: &str, name: &str, version: &str) -> Document {
    Document {
        kind:     kind.to_string(),
        name:     name.to_string(),
        version:  version.to_string(),
        metadata: DocumentMetadata {
            namespace:   None,
            owner:       None,
            description: None,
            tags:        vec![],
            extra:       HashMap::new(),
        },
        spec:   json!({}),
        status: None,
    }
}

/// Builds a [`Document`] with a custom spec value so equality checks are meaningful.
fn make_doc_with_spec(kind: &str, name: &str, version: &str, spec: serde_json::Value) -> Document {
    let mut d  = make_doc(kind, name, version);
    d.spec = spec;
    d
}

/// Returns a parser map that handles `"yaml"` extensions.
fn yaml_extensions() -> HashMap<String, Box<dyn DocumentParser>> {
    let mut m: HashMap<String, Box<dyn DocumentParser>> = HashMap::new();
    m.insert("yaml".to_string(), Box::new(YamlDocumentParser));
    m
}

/// Returns a minimal YAML string that deserialises to a [`Document`].
///
/// The [`YamlDocumentParser`] uses `serde_yaml::Deserializer::from_str`, which
/// iterates over top-level YAML documents in the stream.  Each top-level entry
/// must therefore be a plain mapping (`kind: ...`), **not** a sequence element.
fn doc_yaml(kind: &str, name: &str, version: &str) -> String {
    format!(
        "kind: {kind}\nname: {name}\nversion: \"{version}\"\nmetadata: {{}}\nspec: {{}}\n",
        kind    = kind,
        name    = name,
        version = version,
    )
}

// ─── Document identity ────────────────────────────────────────────────────────

#[test]
fn doc_key_format() {
    let doc = make_doc("task/Shell:1.0", "my-task", "1.0.0");
    assert_eq!(doc_key(&doc), "task/Shell:1.0/my-task");
}

#[test]
fn docs_equal_when_identical() {
    let a = make_doc("k", "n", "1.0.0");
    let b = make_doc("k", "n", "1.0.0");
    assert!(docs_equal(&a, &b));
}

#[test]
fn docs_equal_false_on_version_change() {
    let a = make_doc("k", "n", "1.0.0");
    let b = make_doc("k", "n", "2.0.0");
    assert!(!docs_equal(&a, &b));
}

#[test]
fn docs_equal_false_on_spec_change() {
    let a = make_doc_with_spec("k", "n", "1.0.0", json!({"cmd": "echo hi"}));
    let b = make_doc_with_spec("k", "n", "1.0.0", json!({"cmd": "echo bye"}));
    assert!(!docs_equal(&a, &b));
}

// ─── File-level event generation ──────────────────────────────────────────────

#[test]
fn events_for_new_file_generates_added_events() {
    let docs = vec![make_doc("k", "a", "1"), make_doc("k", "b", "1")];
    let events = events_for_new_file(&docs, "/entry", "/file.yaml");

    assert_eq!(events.len(), 2);
    for event in events {
        assert!(matches!(event, LocalFsChangeEvent::DocumentAdded { .. }));
    }
}

#[test]
fn events_for_deleted_file_generates_removed_events() {
    let loaded = LocalFsLoadedFile {
        path:        "/file.yaml".to_string(),
        last_parsed: SystemTime::now(),
        documents:   vec![make_doc("k", "a", "1"), make_doc("k", "b", "1")],
    };
    let events = events_for_deleted_file(&loaded, "/entry");

    assert_eq!(events.len(), 2);
    for event in events {
        assert!(matches!(event, LocalFsChangeEvent::DocumentRemoved { .. }));
    }
}

// ─── Document-level diffing ───────────────────────────────────────────────────

#[test]
fn diff_no_events_when_nothing_changed() {
    let docs   = vec![make_doc("k", "a", "1")];
    let events = diff_modified_file(&docs, &docs, "/entry", "/file.yaml");
    assert!(events.is_empty());
}

#[test]
fn diff_added_event_for_new_document() {
    let old_docs = vec![make_doc("k", "a", "1")];
    let new_docs = vec![make_doc("k", "a", "1"), make_doc("k", "b", "1")];
    let events   = diff_modified_file(&old_docs, &new_docs, "/entry", "/file.yaml");

    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], LocalFsChangeEvent::DocumentAdded { .. }));
}

#[test]
fn diff_removed_event_for_missing_document() {
    let old_docs = vec![make_doc("k", "a", "1"), make_doc("k", "b", "1")];
    let new_docs = vec![make_doc("k", "a", "1")];
    let events   = diff_modified_file(&old_docs, &new_docs, "/entry", "/file.yaml");

    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], LocalFsChangeEvent::DocumentRemoved { .. }));
}

#[test]
fn diff_modified_event_for_changed_document() {
    let old_docs = vec![make_doc_with_spec("k", "a", "1", json!({"v": 1}))];
    let new_docs = vec![make_doc_with_spec("k", "a", "1", json!({"v": 2}))];
    let events   = diff_modified_file(&old_docs, &new_docs, "/entry", "/file.yaml");

    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], LocalFsChangeEvent::DocumentModified { .. }));
}

#[test]
fn diff_mixed_scenario() {
    // a: modified,  b: removed,  c: added
    let old_docs = vec![
        make_doc_with_spec("k", "a", "1", json!({"v": 1})),
        make_doc("k", "b", "1"),
    ];
    let new_docs = vec![
        make_doc_with_spec("k", "a", "1", json!({"v": 2})),
        make_doc("k", "c", "1"),
    ];
    let events = diff_modified_file(&old_docs, &new_docs, "/entry", "/file.yaml");

    let added    = events.iter().filter(|e| matches!(e, LocalFsChangeEvent::DocumentAdded   { .. })).count();
    let modified = events.iter().filter(|e| matches!(e, LocalFsChangeEvent::DocumentModified{ .. })).count();
    let removed  = events.iter().filter(|e| matches!(e, LocalFsChangeEvent::DocumentRemoved { .. })).count();

    assert_eq!(added,    1, "expected 1 Added");
    assert_eq!(modified, 1, "expected 1 Modified");
    assert_eq!(removed,  1, "expected 1 Removed");
}

// ─── Extension parsing ────────────────────────────────────────────────────────

#[test]
fn get_extension_lowercase() {
    assert_eq!(get_extension("file.YAML"), Some("yaml".to_string()));
}

#[test]
fn get_extension_none_for_no_extension() {
    assert_eq!(get_extension("file"), None);
}

#[test]
fn get_extension_empty_path() {
    assert_eq!(get_extension(""), None);
}

// ─── File collection ──────────────────────────────────────────────────────────

#[test]
fn collect_files_finds_single_supported_file() {
    let dir = TempDir::new();
    let p   = dir.write("test.yaml", &doc_yaml("k", "n", "1"));
    let ext = yaml_extensions();

    let files = collect_files(p.to_str().unwrap(), false, &ext);
    assert_eq!(files.len(), 1);
    assert!(files.contains_key(p.to_str().unwrap()));
}

#[test]
fn collect_files_skips_unsupported_extension() {
    let dir = TempDir::new();
    dir.write("test.txt", "hello");
    let ext = yaml_extensions();

    let files = collect_files(dir.path().to_str().unwrap(), false, &ext);
    assert!(files.is_empty(), "txt files should be ignored");
}

#[test]
fn collect_files_returns_empty_for_missing_path() {
    let ext   = yaml_extensions();
    let files = collect_files("/no/such/path/xyz", false, &ext);
    assert!(files.is_empty(), "should not panic or fail for missing path");
}

#[test]
fn collect_files_directory_lists_direct_children() {
    let dir = TempDir::new();
    dir.write("a.yaml", &doc_yaml("k", "a", "1"));
    dir.write("b.yaml", &doc_yaml("k", "b", "1"));
    dir.write("c.txt",  "ignored");
    let ext = yaml_extensions();

    let files = collect_files(dir.path().to_str().unwrap(), false, &ext);
    assert_eq!(files.len(), 2);
}

#[test]
fn collect_files_non_recursive_ignores_subdirectories() {
    let dir    = TempDir::new();
    let subdir = dir.child("sub");
    std::fs::create_dir_all(&subdir).unwrap();
    dir.write("root.yaml",        &doc_yaml("k", "root", "1"));
    std::fs::write(subdir.join("nested.yaml"), doc_yaml("k", "nested", "1")).unwrap();
    let ext = yaml_extensions();

    let files = collect_files(dir.path().to_str().unwrap(), false, &ext);
    assert_eq!(files.len(), 1, "recursive=false must not descend into sub/");
}

#[test]
fn collect_files_recursive_finds_nested_files() {
    let dir    = TempDir::new();
    let subdir = dir.child("sub");
    std::fs::create_dir_all(&subdir).unwrap();
    dir.write("root.yaml",        &doc_yaml("k", "root", "1"));
    std::fs::write(subdir.join("nested.yaml"), doc_yaml("k", "nested", "1")).unwrap();
    let ext = yaml_extensions();

    let files = collect_files(dir.path().to_str().unwrap(), true, &ext);
    assert_eq!(files.len(), 2, "recursive=true must find files in sub/");
}

// ─── File parsing ─────────────────────────────────────────────────────────────

#[test]
fn parse_file_returns_documents_from_yaml() {
    let dir  = TempDir::new();
    let path = dir.write("doc.yaml", &doc_yaml("task/Shell:1.0", "my-task", "1.0.0"));
    let ext  = yaml_extensions();

    let docs = parse_file(path.to_str().unwrap(), &ext).unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].kind, "task/Shell:1.0");
    assert_eq!(docs[0].name, "my-task");
}

#[test]
fn parse_file_errors_on_unsupported_extension() {
    let dir  = TempDir::new();
    let path = dir.write("doc.txt", "irrelevant");
    let ext  = yaml_extensions();

    assert!(parse_file(path.to_str().unwrap(), &ext).is_err());
}

#[test]
fn parse_file_errors_on_invalid_yaml() {
    let dir  = TempDir::new();
    let path = dir.write("bad.yaml", "{{{{not valid yaml");
    let ext  = yaml_extensions();

    assert!(parse_file(path.to_str().unwrap(), &ext).is_err());
}

// ─── Scanner: full entry scan lifecycle ───────────────────────────────────────

fn make_entry(dir: &TempDir, recursive: bool) -> LocalFsEntry {
    LocalFsEntry {
        path:         dir.path().to_str().unwrap().to_string(),
        recursive,
        watch:        false,
        loaded_files: HashMap::new(),
    }
}

#[test]
fn scan_entry_initial_scan_emits_added_events() {
    let dir = TempDir::new();
    dir.write("a.yaml", &doc_yaml("k", "a", "1"));
    dir.write("b.yaml", &doc_yaml("k", "b", "1"));
    let ext = yaml_extensions();

    let mut entry  = make_entry(&dir, false);
    let     events = scan_entry(&mut entry, &ext);

    let added: Vec<_> = events.iter()
        .filter(|e| matches!(e, LocalFsChangeEvent::DocumentAdded { .. }))
        .collect();
    assert_eq!(added.len(), 2, "first scan should yield DocumentAdded for each document");
}

#[test]
fn scan_entry_second_scan_with_no_changes_is_silent() {
    let dir = TempDir::new();
    dir.write("a.yaml", &doc_yaml("k", "a", "1"));
    let ext = yaml_extensions();

    let mut entry = make_entry(&dir, false);
    scan_entry(&mut entry, &ext); // initial scan populates cache

    let events = scan_entry(&mut entry, &ext); // nothing changed
    assert!(events.is_empty(), "no events expected when nothing changed");
}

#[test]
fn scan_entry_detects_deleted_file() {
    let dir  = TempDir::new();
    let path = dir.write("a.yaml", &doc_yaml("k", "a", "1"));
    let ext  = yaml_extensions();

    let mut entry = make_entry(&dir, false);
    scan_entry(&mut entry, &ext); // initial scan

    std::fs::remove_file(&path).unwrap();
    let events = scan_entry(&mut entry, &ext);

    let removed: Vec<_> = events.iter()
        .filter(|e| matches!(e, LocalFsChangeEvent::DocumentRemoved { .. }))
        .collect();
    assert_eq!(removed.len(), 1, "deleting the file should produce a DocumentRemoved event");
}

#[test]
fn scan_entry_detects_modified_document() {
    let dir  = TempDir::new();
    let path = dir.write("a.yaml", &doc_yaml("k", "a", "1"));
    let ext  = yaml_extensions();

    let mut entry = make_entry(&dir, false);
    scan_entry(&mut entry, &ext); // initial scan

    // Backdate the cached mtime so the scanner treats the file as modified.
    let old_time = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
    entry.loaded_files.get_mut(path.to_str().unwrap()).unwrap().last_parsed = old_time;

    // Write a different document to the file.
    std::fs::write(&path, doc_yaml("k", "a", "2")).unwrap();
    let events = scan_entry(&mut entry, &ext);

    let modified: Vec<_> = events.iter()
        .filter(|e| matches!(e, LocalFsChangeEvent::DocumentModified { .. }))
        .collect();
    assert_eq!(modified.len(), 1, "changing the doc version should produce a DocumentModified event");
}

#[test]
fn scan_entry_cache_is_empty_after_all_files_deleted() {
    let dir  = TempDir::new();
    let path = dir.write("a.yaml", &doc_yaml("k", "a", "1"));
    let ext  = yaml_extensions();

    let mut entry = make_entry(&dir, false);
    scan_entry(&mut entry, &ext);
    assert!(!entry.loaded_files.is_empty());

    std::fs::remove_file(&path).unwrap();
    scan_entry(&mut entry, &ext);
    assert!(entry.loaded_files.is_empty(), "cache must be empty after all files are gone");
}

// ─── Loader: load() aggregates documents from all entries ─────────────────────

#[test]
fn loader_load_returns_all_cached_documents() {
    use workaholic::DocumentLoader;

    let loader = {
        let mut l = LocalFsLoader::new(yaml_extensions());

        // Manually pre-populate the cache for two entries.
        let entry_a = LocalFsEntry {
            path:      "/virtual/a".to_string(),
            recursive: false,
            watch:     false,
            loaded_files: {
                let mut m = HashMap::new();
                m.insert(
                    "/virtual/a/x.yaml".to_string(),
                    LocalFsLoadedFile {
                        path:        "/virtual/a/x.yaml".to_string(),
                        last_parsed: SystemTime::now(),
                        documents:   vec![make_doc("k", "x", "1")],
                    },
                );
                m
            },
        };
        let entry_b = LocalFsEntry {
            path:      "/virtual/b".to_string(),
            recursive: false,
            watch:     false,
            loaded_files: {
                let mut m = HashMap::new();
                m.insert(
                    "/virtual/b/y.yaml".to_string(),
                    LocalFsLoadedFile {
                        path:        "/virtual/b/y.yaml".to_string(),
                        last_parsed: SystemTime::now(),
                        documents:   vec![make_doc("k", "y", "1"), make_doc("k", "z", "1")],
                    },
                );
                m
            },
        };
        l.entries.push(Arc::new(Mutex::new(entry_a)));
        l.entries.push(Arc::new(Mutex::new(entry_b)));
        l
    };

    let docs = loader.load().unwrap();
    assert_eq!(docs.len(), 3, "load() should return documents from all entries");
}
