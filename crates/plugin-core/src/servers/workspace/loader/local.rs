//! Local filesystem loader.
//!
//! Scans a directory recursively for `*.yaml` / `*.yml` files, parses each as
//! a multi-document YAML file, and watches for changes using the `notify`
//! crate (inotify on Linux, FSEvents on macOS, ReadDirectoryChanges on Windows).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use notify::{Event, EventKind, PollWatcher, RecursiveMode, Watcher};
use orkester_common::{log_debug, log_error, log_info, log_warn};
use tokio::sync::mpsc::UnboundedSender;

use super::super::model::ObjectEnvelope;
use super::{parse_yaml_documents, LoaderError, LoaderEvent, ObjectLoader};

// ── LocalLoader ───────────────────────────────────────────────────────────────

pub struct LocalLoader {
    dir: PathBuf,
    /// Per-file object index.  Populated by `load_all()` and kept up-to-date
    /// by `watch()` so that we can diff old vs new on every modify event and
    /// emit `Removed` for objects that have disappeared from a file.
    file_index: Arc<Mutex<HashMap<PathBuf, Vec<ObjectEnvelope>>>>,
}

impl LocalLoader {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            file_index: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl ObjectLoader for LocalLoader {
    async fn load_all(&self) -> Result<Vec<ObjectEnvelope>, LoaderError> {
        let dir = self.dir.clone();
        // Run the blocking fs scan on the threadpool.
        let indexed: HashMap<PathBuf, Vec<ObjectEnvelope>> =
            tokio::task::spawn_blocking(move || scan_dir_indexed(&dir))
                .await
                .map_err(|e| LoaderError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))??;

        let mut all = Vec::new();
        let mut index = self.file_index.lock().unwrap();
        for (path, objs) in indexed {
            all.extend(objs.clone());
            index.insert(path, objs);
        }
        Ok(all)
    }

    async fn watch(&self, tx: UnboundedSender<LoaderEvent>) {
        let dir = self.dir.clone();
        let file_index = self.file_index.clone();

        // `notify` uses OS-level events; we bridge them onto a std channel then
        // forward to the Tokio unbounded sender in a blocking thread.
        std::thread::spawn(move || {
            let (notify_tx, notify_rx) = std::sync::mpsc::channel::<notify::Result<Event>>();

            let mut watcher = match PollWatcher::new(
                notify_tx,
                notify::Config::default()
                    .with_poll_interval(Duration::from_secs(2)),
            ) {
                Ok(w) => w,
                Err(e) => {
                    log_error!("Failed to create fs watcher: {}", e);
                    return;
                }
            };

            if let Err(e) = watcher.watch(&dir, RecursiveMode::Recursive) {
                log_error!(
                    "Failed to watch directory '{}': {}",
                    dir.display(),
                    e
                );
                return;
            }

            log_info!(
                "Watching '{}' for changes.",
                dir.display()
            );

            for event_result in notify_rx {
                match event_result {
                    Ok(event) => handle_event(event, &tx, &file_index),
                    Err(e) => log_warn!("Watch error: {}", e),
                }
            }

            log_info!("Watcher stopped.");
        });
    }
}

// ── FS helpers ────────────────────────────────────────────────────────────────

/// Recursively scan `dir`, returning a map of `path → objects` so callers can
/// track which objects came from which file.
fn scan_dir_indexed(dir: &Path) -> Result<HashMap<PathBuf, Vec<ObjectEnvelope>>, LoaderError> {
    let mut result: HashMap<PathBuf, Vec<ObjectEnvelope>> = HashMap::new();

    fn visit(path: &Path, result: &mut HashMap<PathBuf, Vec<ObjectEnvelope>>) -> Result<(), LoaderError> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                visit(&entry_path, result)?;
            } else if is_yaml(&entry_path) {
                match load_file(&entry_path) {
                    Ok(objs) => {
                        log_debug!(
                            "Loaded {} object(s) from '{}'",
                            objs.len(),
                            entry_path.display()
                        );
                        result.insert(entry_path, objs);
                    }
                    Err(e) => {
                        log_warn!("Skipping '{}': {}", entry_path.display(), e);
                    }
                }
            }
        }
        Ok(())
    }

    visit(dir, &mut result)?;
    Ok(result)
}

fn load_file(path: &Path) -> Result<Vec<ObjectEnvelope>, LoaderError> {
    let content = std::fs::read_to_string(path)?;
    let path_str = path.to_string_lossy();
    parse_yaml_documents(&content, &path_str)
}

fn is_yaml(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("yaml" | "yml")
    )
}

// ── Event handling ────────────────────────────────────────────────────────────

/// Stable identity key for an object: `kind/namespace/name@version`.
fn identity_key(obj: &ObjectEnvelope) -> String {
    match obj {
        ObjectEnvelope::Namespace(n) =>
            format!("Namespace//{}//{}", n.meta.name, n.meta.version),
        ObjectEnvelope::Task(t) =>
            format!("Task/{}/{}/{}", t.meta.metadata.namespace, t.meta.name, t.meta.version),
        ObjectEnvelope::Work(w) =>
            format!("Work/{}/{}/{}", w.meta.metadata.namespace, w.meta.name, w.meta.version),
    }
}

fn handle_event(
    event: Event,
    tx: &UnboundedSender<LoaderEvent>,
    file_index: &Mutex<HashMap<PathBuf, Vec<ObjectEnvelope>>>,
) {
    let yaml_paths: Vec<PathBuf> = event
        .paths
        .iter()
        .filter(|p| is_yaml(p))
        .cloned()
        .collect();

    if yaml_paths.is_empty() {
        return;
    }

    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => {
            for path in yaml_paths {
                let new_objs = match load_file(&path) {
                    Ok(objs) => objs,
                    Err(e) => {
                        log_warn!("Error reloading '{}': {}", path.display(), e);
                        continue;
                    }
                };

                let mut index = file_index.lock().unwrap();
                let old_objs = index.get(&path).cloned().unwrap_or_default();

                // Emit Removed for objects present in the old version but gone
                // from the new one (document deleted or renamed inside the file).
                for old in &old_objs {
                    if !new_objs.iter().any(|n| identity_key(n) == identity_key(old)) {
                        log_info!(
                            "Object removed from '{}': {} '{}'",
                            path.display(), old.kind(), old.name()
                        );
                        let _ = tx.send(LoaderEvent::Removed(old.clone()));
                    }
                }

                // Upsert every object currently in the file.
                log_info!("File changed: '{}' ({} object(s))", path.display(), new_objs.len());
                for obj in &new_objs {
                    let _ = tx.send(LoaderEvent::Upserted(obj.clone()));
                }

                index.insert(path, new_objs);
            }
        }
        EventKind::Remove(_) => {
            for path in yaml_paths {
                let mut index = file_index.lock().unwrap();
                if let Some(old_objs) = index.remove(&path) {
                    log_info!(
                        "File removed: '{}' ({} object(s) deleted)",
                        path.display(), old_objs.len()
                    );
                    for obj in old_objs {
                        let _ = tx.send(LoaderEvent::Removed(obj));
                    }
                } else {
                    log_info!("File removed: '{}'", path.display());
                }
            }
        }
        _ => {}
    }
}
