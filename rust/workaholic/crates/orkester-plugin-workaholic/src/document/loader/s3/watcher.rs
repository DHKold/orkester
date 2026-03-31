//! Background S3 watcher: polls the bucket at a configurable interval.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use orkester_plugin::{log_error, log_info, log_trace};

use chrono::Utc;
use workaholic::DocumentParser;

use super::scanner::scan_entry;
use super::types::{S3ChangeEvent, S3Entry, S3ScanMetrics};

const METRICS_CAPACITY: usize = 200;

fn push_metrics(store: &Arc<Mutex<VecDeque<S3ScanMetrics>>>, m: S3ScanMetrics) {
    let mut s = store.lock().unwrap();
    if s.len() >= METRICS_CAPACITY { s.pop_front(); }
    s.push_back(m);
}

pub fn build_metrics_pub(entry_id: String, is_initial: bool, started: Instant, events: &[S3ChangeEvent]) -> S3ScanMetrics {
    S3ScanMetrics {
        entry_id,
        scanned_at:      Utc::now().to_rfc3339(),
        is_initial,
        duration_ms:     started.elapsed().as_millis() as u64,
        events_added:    events.iter().filter(|e| matches!(e, S3ChangeEvent::DocumentAdded    { .. })).count(),
        events_modified: events.iter().filter(|e| matches!(e, S3ChangeEvent::DocumentModified { .. })).count(),
        events_removed:  events.iter().filter(|e| matches!(e, S3ChangeEvent::DocumentRemoved  { .. })).count(),
    }
}

fn watcher_loop(
    entry_arc:     Arc<Mutex<S3Entry>>,
    extensions:    Arc<HashMap<String, Box<dyn DocumentParser>>>,
    metrics_store: Arc<Mutex<VecDeque<S3ScanMetrics>>>,
    on_event:      impl Fn(S3ChangeEvent),
    on_metrics:    impl Fn(&S3ScanMetrics),
    poll_secs:     u64,
) {
    loop {
        std::thread::sleep(Duration::from_secs(poll_secs));
        let started  = Instant::now();
        let entry_id = entry_arc.lock().map(|e| format!("s3://{}/{}", e.config.bucket, e.config.prefix)).unwrap_or_default();
        let events = match entry_arc.lock() {
            Ok(mut e) => scan_entry(&mut e, &extensions),
            Err(err)  => { log_error!("S3 watcher mutex poisoned: {err}"); return; }
        };
        let m = build_metrics_pub(entry_id, false, started, &events);
        if m.events_added + m.events_modified + m.events_removed > 0 {
            log_info!("[loader/s3] poll '{}': {}ms +{} ~{} -{}", m.entry_id, m.duration_ms, m.events_added, m.events_modified, m.events_removed);
        } else {
            log_trace!("[loader/s3] poll '{}': {}ms no changes", m.entry_id, m.duration_ms);
        }
        push_metrics(&metrics_store, m.clone());
        on_metrics(&m);
        for event in events { on_event(event); }
    }
}

/// Spawn a background watcher thread for one S3 entry.
/// `on_metrics` is called after every scan so scan counters can be forwarded
/// to the metrics server via fire-and-forget envelopes.
pub fn spawn_entry_watcher(
    entry_arc:     Arc<Mutex<S3Entry>>,
    extensions:    Arc<HashMap<String, Box<dyn DocumentParser>>>,
    metrics_store: Arc<Mutex<VecDeque<S3ScanMetrics>>>,
    poll_secs:     u64,
    on_event:      impl Fn(S3ChangeEvent) + Send + 'static,
    on_metrics:    impl Fn(&S3ScanMetrics) + Send + 'static,
) {
    std::thread::spawn(move || {
        watcher_loop(entry_arc, extensions, metrics_store, on_event, on_metrics, poll_secs);
    });
}
