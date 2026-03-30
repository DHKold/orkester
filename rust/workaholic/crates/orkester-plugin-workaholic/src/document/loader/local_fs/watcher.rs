//! Background watcher: spawns a thread that polls a single `LocalFsEntry` for
//! filesystem changes and forwards events through a caller-supplied callback.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::Utc;
use workaholic::DocumentParser;

use super::scanner::scan_entry;
use super::types::{LocalFsChangeEvent, LocalFsEntry, LocalFsScanMetrics};

/// How often the background thread wakes up to check for file changes.
const POLL_INTERVAL: Duration = Duration::from_secs(5);
/// Maximum number of metrics records stored (oldest are dropped when full).
const METRICS_CAPACITY: usize = 200;

// ─── Internal loop ─────────────────────────────────────────────────────────────

/// Runs a blocking poll loop, calling `on_event` for every detected change.
///
/// Exits only if the mutex becomes poisoned (which indicates a programming error
/// and there is no safe way to continue).
fn watcher_loop(
    entry_arc:     Arc<Mutex<LocalFsEntry>>,
    extensions:    Arc<HashMap<String, Box<dyn DocumentParser>>>,
    metrics_store: Arc<Mutex<VecDeque<LocalFsScanMetrics>>>,
    on_event:      impl Fn(LocalFsChangeEvent),
    on_metrics:    impl Fn(&LocalFsScanMetrics),
) {
    loop {
        std::thread::sleep(POLL_INTERVAL);

        let started    = Instant::now();
        let scanned_at = Utc::now().to_rfc3339();
        let entry_path = entry_arc.lock().map(|e| e.path.clone()).unwrap_or_default();

        let events = match entry_arc.lock() {
            Ok(mut entry) => scan_entry(&mut entry, &extensions),
            Err(e)        => { log::error!("Watcher mutex poisoned: {}", e); return; }
        };

        let duration_ms = started.elapsed().as_millis() as u64;
        let m = LocalFsScanMetrics {
            entry_path,
            scanned_at,
            is_initial:      false,
            duration_ms,
            events_added:    events.iter().filter(|e| matches!(e, LocalFsChangeEvent::DocumentAdded { .. })).count(),
            events_modified: events.iter().filter(|e| matches!(e, LocalFsChangeEvent::DocumentModified { .. })).count(),
            events_removed:  events.iter().filter(|e| matches!(e, LocalFsChangeEvent::DocumentRemoved { .. })).count(),
        };
        if m.events_added + m.events_modified + m.events_removed > 0 {
            eprintln!(
                "[loader] watch '{}': {}ms, +{} ~{} -{}",
                m.entry_path, m.duration_ms, m.events_added, m.events_modified, m.events_removed,
            );
        }
        {
            let mut store = metrics_store.lock().unwrap();
            if store.len() >= METRICS_CAPACITY { store.pop_front(); }
            store.push_back(m.clone());
        }
        on_metrics(&m);

        for event in events {
            on_event(event);
        }
    }
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Spawns a background thread that polls `entry_arc` every [`POLL_INTERVAL`].
///
/// `on_event` is called for every change detected; `on_metrics` is called after
/// every scan so the caller can forward counters to an external metrics store.
/// Both must be `Send + 'static` because they are moved into the thread.
pub fn spawn_entry_watcher(
    entry_arc:     Arc<Mutex<LocalFsEntry>>,
    extensions:    Arc<HashMap<String, Box<dyn DocumentParser>>>,
    metrics_store: Arc<Mutex<VecDeque<LocalFsScanMetrics>>>,
    on_event:      impl Fn(LocalFsChangeEvent) + Send + 'static,
    on_metrics:    impl Fn(&LocalFsScanMetrics) + Send + 'static,
) {
    std::thread::spawn(move || watcher_loop(entry_arc, extensions, metrics_store, on_event, on_metrics));
}
