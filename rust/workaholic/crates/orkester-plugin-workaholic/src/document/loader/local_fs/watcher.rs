//! Background watcher: spawns a thread that polls a single `LocalFsEntry` for
//! filesystem changes and forwards events through a caller-supplied callback.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use workaholic::DocumentParser;

use super::scanner::scan_entry;
use super::types::{LocalFsChangeEvent, LocalFsEntry};

/// How often the background thread wakes up to check for file changes.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

// ─── Internal loop ─────────────────────────────────────────────────────────────

/// Runs a blocking poll loop, calling `on_event` for every detected change.
///
/// Exits only if the mutex becomes poisoned (which indicates a programming error
/// and there is no safe way to continue).
fn watcher_loop(
    entry_arc: Arc<Mutex<LocalFsEntry>>,
    extensions: Arc<HashMap<String, Box<dyn DocumentParser>>>,
    on_event: impl Fn(LocalFsChangeEvent),
) {
    loop {
        std::thread::sleep(POLL_INTERVAL);

        let events = match entry_arc.lock() {
            Ok(mut entry) => scan_entry(&mut entry, &extensions),
            Err(e)        => { log::error!("Watcher mutex poisoned: {}", e); return; }
        };

        for event in events {
            on_event(event);
        }
    }
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Spawns a background thread that polls `entry_arc` every [`POLL_INTERVAL`].
///
/// `on_event` is called from the background thread for every change detected.
/// It must be `Send + 'static` because it is moved into the thread.
pub fn spawn_entry_watcher(
    entry_arc: Arc<Mutex<LocalFsEntry>>,
    extensions: Arc<HashMap<String, Box<dyn DocumentParser>>>,
    on_event: impl Fn(LocalFsChangeEvent) + Send + 'static,
) {
    std::thread::spawn(move || watcher_loop(entry_arc, extensions, on_event));
}
