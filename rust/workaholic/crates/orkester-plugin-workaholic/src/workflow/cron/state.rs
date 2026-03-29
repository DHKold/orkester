//! Per-cron runtime state maintained by the scheduler.

use std::collections::HashMap;
use workaholic::{CronDoc, Trigger};

/// In-memory state the scheduler keeps for one active `CronDoc`.
#[derive(Debug, Clone)]
pub struct CronEntry {
    pub cron:       CronDoc,
    /// The UTC timestamp (ms since epoch) when this cron should next fire.
    pub next_at_ms: Option<i64>,
    /// Generation counter incremented when the entry is updated.  Used to
    /// invalidate stale occurrences from the priority queue.
    pub generation: u64,
}

impl CronEntry {
    pub fn new(cron: CronDoc, next_at_ms: Option<i64>) -> Self {
        Self { cron, next_at_ms, generation: 0 }
    }

    pub fn advance(&mut self, next_at_ms: Option<i64>) {
        self.next_at_ms = next_at_ms;
        self.generation = self.generation.wrapping_add(1);
    }
}

/// Collection of all registered crons.
#[derive(Debug, Default)]
pub struct CronSchedulerState {
    pub entries: HashMap<String, CronEntry>,
}

impl CronSchedulerState {
    pub fn insert(&mut self, entry: CronEntry) {
        self.entries.insert(entry.cron.name.clone(), entry);
    }

    pub fn remove(&mut self, name: &str) {
        self.entries.remove(name);
    }

    pub fn get_all_crons(&self) -> Vec<CronDoc> {
        self.entries.values().map(|e| e.cron.clone()).collect()
    }
}

/// Build a `Trigger` for a cron firing.
pub fn cron_trigger(cron_name: &str, fired_at: &str) -> Trigger {
    Trigger {
        trigger_type: "cron".to_string(),
        at:           Some(fired_at.to_string()),
        identity:     Some(format!("cron:{}", cron_name)),
    }
}
