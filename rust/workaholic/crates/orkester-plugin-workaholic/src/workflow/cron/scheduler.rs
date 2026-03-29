//! Cron scheduler — background thread that fires triggers on schedule.
//!
//! The scheduler uses a priority queue keyed by `next_at_ms` so it sleeps
//! precisely until the next due cron.  A control channel wakes it early to
//! handle register/unregister/shutdown events.
//!
//! Fire semantics:
//! * Each firing produces a `(CronDoc, Trigger)` pair in an output channel.
//! * The caller (usually the WorkflowServer) drains that channel and resolves
//!   the trigger into a full WorkRunRequest.

use std::collections::BinaryHeap;
use std::cmp::Reverse;
use std::time::Duration;

use chrono::{DateTime, Datelike, Timelike, Utc};
use crossbeam_channel::Sender;
use workaholic::CronDoc;

use super::control::{CronControl, CronControlEvent};
use super::state::{CronEntry, CronSchedulerState, cron_trigger};

// ─── CronScheduler ────────────────────────────────────────────────────────────

/// Manages active cron definitions and fires triggers on schedule.
pub struct CronScheduler {
    control:  CronControl,
    fire_tx:  Sender<(CronDoc, workaholic::Trigger)>,
}

impl CronScheduler {
    /// Start the scheduler background thread.
    ///
    /// Returns a `CronScheduler` (control handle) and a `Receiver` through
    /// which fired `(CronDoc, Trigger)` pairs can be consumed.
    pub fn start() -> (Self, crossbeam_channel::Receiver<(CronDoc, workaholic::Trigger)>) {
        let (ctrl_tx, ctrl_rx) = crossbeam_channel::unbounded();
        let (fire_tx, fire_rx) = crossbeam_channel::unbounded();

        let fire_tx_clone = fire_tx.clone();
        std::thread::spawn(move || scheduler_loop(ctrl_rx, fire_tx_clone));

        let scheduler = Self {
            control: CronControl::new(ctrl_tx),
            fire_tx,
        };
        (scheduler, fire_rx)
    }

    pub fn register(&self, cron: CronDoc) {
        self.control.register(cron);
    }

    pub fn unregister(&self, name: impl Into<String>) {
        self.control.unregister(name);
    }

    pub fn shutdown(&self) {
        self.control.shutdown();
    }
}

// ─── Background loop ──────────────────────────────────────────────────────────

/// A heap entry: `(next_at_ms, cron_name, generation)` ordered by earliest-first.
type HeapEntry = Reverse<(i64, String, u64)>;

fn scheduler_loop(
    ctrl_rx: crossbeam_channel::Receiver<CronControlEvent>,
    fire_tx: Sender<(CronDoc, workaholic::Trigger)>,
) {
    let mut state = CronSchedulerState::default();
    let mut heap: BinaryHeap<HeapEntry> = BinaryHeap::new();

    loop {
        // Drain control events without blocking.
        loop {
            match ctrl_rx.try_recv() {
                Ok(CronControlEvent::Register(cron))   => handle_register(&mut state, &mut heap, cron),
                Ok(CronControlEvent::Unregister { name }) => { state.remove(&name); }
                Ok(CronControlEvent::Shutdown)           => return,
                Err(_)                                   => break,
            }
        }

        // Find the earliest due cron.
        let now_ms = Utc::now().timestamp_millis();

        // Fire all crons whose next_at_ms has arrived.
        let mut fired = false;
        while let Some(Reverse((next_ms, name, entry_gen))) = heap.peek().cloned() {
            if next_ms > now_ms { break; }
            heap.pop();
            if let Some(entry) = state.entries.get_mut(&name) {
                if entry.generation != entry_gen { continue; } // stale
                let fired_at = ms_to_iso(next_ms);
                let trigger  = cron_trigger(&name, &fired_at);
                let _ = fire_tx.send((entry.cron.clone(), trigger));
                // Advance to the next occurrence.
                let next = next_occurrence_after(&entry.cron, next_ms + 1);
                entry.advance(next);
                if let Some(nxt) = next {
                    heap.push(Reverse((nxt, name.clone(), entry.generation)));
                }
                fired = true;
            }
        }

        // Sleep until the next due cron or up to 1 second for control events.
        let sleep_ms = if let Some(Reverse((next_ms, _, _))) = heap.peek() {
            let now_ms = Utc::now().timestamp_millis();
            (*next_ms - now_ms).max(0).min(1000) as u64
        } else {
            1000
        };

        if !fired {
            let _ = ctrl_rx.recv_timeout(Duration::from_millis(sleep_ms));
        }
    }
}

fn handle_register(
    state: &mut CronSchedulerState,
    heap:  &mut BinaryHeap<HeapEntry>,
    cron:  CronDoc,
) {
    let now_ms = Utc::now().timestamp_millis();
    let next   = next_occurrence_after(&cron, now_ms);
    let mut entry = CronEntry::new(cron.clone(), next);

    // If there's an existing entry, bump generation so heap entries go stale.
    if let Some(existing) = state.entries.get(&cron.name) {
        entry.generation = existing.generation.wrapping_add(1);
    }

    if let Some(nxt) = next {
        heap.push(Reverse((nxt, cron.name.clone(), entry.generation)));
    }
    state.insert(entry);
}

// ─── Cron expression parser ───────────────────────────────────────────────────

/// Return the next fire time (ms since epoch) for `cron` after `after_ms`.
fn next_occurrence_after(cron: &CronDoc, after_ms: i64) -> Option<i64> {
    if !cron.spec.enabled { return None; }

    cron.spec.schedules.iter()
        .filter_map(|expr| parse_next(expr, after_ms))
        .min()
}

/// Parse a standard 5-field cron expression and compute the next occurrence.
///
/// Fields: minute hour day-of-month month day-of-week
/// Wildcard `*` means "any".  Ranges and lists are not supported.
fn parse_next(expr: &str, after_ms: i64) -> Option<i64> {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 5 { return None; }

    let minute_field = parts[0];
    let hour_field   = parts[1];
    let dom_field    = parts[2];
    let month_field  = parts[3];
    let dow_field    = parts[4];

    // Start search from the minute after `after_ms`.
    let start = DateTime::<Utc>::from_timestamp_millis(after_ms)? + chrono::Duration::minutes(1);
    // Search up to 4 years ahead.
    let limit = start + chrono::Duration::days(4 * 366);

    let mut candidate = start
        .with_second(0).unwrap()
        .with_nanosecond(0).unwrap();

    while candidate < limit {
        if !field_matches(month_field, candidate.month() as i32) {
            candidate = advance_month(candidate)?;
            continue;
        }
        if !field_matches(dom_field, candidate.day() as i32) {
            candidate = advance_day(candidate)?;
            continue;
        }
        if !field_matches(dow_field, candidate.weekday().num_days_from_sunday() as i32) {
            candidate = advance_day(candidate)?;
            continue;
        }
        if !field_matches(hour_field, candidate.hour() as i32) {
            candidate = advance_hour(candidate)?;
            continue;
        }
        if !field_matches(minute_field, candidate.minute() as i32) {
            candidate = candidate + chrono::Duration::minutes(1);
            continue;
        }
        return Some(candidate.timestamp_millis());
    }
    None
}

fn field_matches(field: &str, value: i32) -> bool {
    if field == "*" { return true; }
    if let Ok(n) = field.parse::<i32>() { return n == value; }
    // Support comma-separated lists.
    field.split(',').any(|part| {
        if let Ok(n) = part.parse::<i32>() { n == value }
        else if let Some((lo, hi)) = part.split_once('-') {
            let lo: i32 = lo.trim().parse().unwrap_or(i32::MAX);
            let hi: i32 = hi.trim().parse().unwrap_or(i32::MIN);
            value >= lo && value <= hi
        } else { false }
    })
}

fn advance_month(dt: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let m = dt.month();
    let y = dt.year();
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    dt.with_year(ny)?.with_month(nm)?.with_day(1)?.with_hour(0)?.with_minute(0)
}

fn advance_day(dt: DateTime<Utc>) -> Option<DateTime<Utc>> {
    (dt + chrono::Duration::days(1)).with_hour(0)?.with_minute(0)
}

fn advance_hour(dt: DateTime<Utc>) -> Option<DateTime<Utc>> {
    (dt + chrono::Duration::hours(1)).with_minute(0)
}

fn ms_to_iso(ms: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(ms)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}
