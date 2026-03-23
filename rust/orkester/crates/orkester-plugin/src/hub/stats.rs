use std::sync::atomic::{AtomicU64, Ordering};

// ── Live counters ─────────────────────────────────────────────────────────────

/// Live, atomically-updated hub metrics.
#[derive(Debug, Default)]
pub struct HubStats {
    /// Envelopes successfully placed in the waiting queue.
    pub submitted:         AtomicU64,
    /// Envelopes rejected due to backpressure.
    pub rejected:          AtomicU64,
    /// Envelopes that matched at least one rule (counted once per envelope).
    pub routed:            AtomicU64,
    /// Envelopes that matched zero rules.
    pub unrouted:          AtomicU64,
    /// Envelopes explicitly dropped by a `drop` target.
    pub dropped:           AtomicU64,
    /// Total dispatcher-level failures (processing continues after each one).
    pub dispatch_failures: AtomicU64,
}

impl HubStats {
    pub fn inc_submitted(&self)         { self.submitted.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_rejected(&self)          { self.rejected.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_routed(&self)            { self.routed.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_unrouted(&self)          { self.unrouted.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_dropped(&self)           { self.dropped.fetch_add(1, Ordering::Relaxed); }
    pub fn inc_dispatch_failures(&self) { self.dispatch_failures.fetch_add(1, Ordering::Relaxed); }

    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            submitted:         self.submitted.load(Ordering::Relaxed),
            rejected:          self.rejected.load(Ordering::Relaxed),
            routed:            self.routed.load(Ordering::Relaxed),
            unrouted:          self.unrouted.load(Ordering::Relaxed),
            dropped:           self.dropped.load(Ordering::Relaxed),
            dispatch_failures: self.dispatch_failures.load(Ordering::Relaxed),
        }
    }
}

// ── Snapshot ──────────────────────────────────────────────────────────────────

/// Immutable point-in-time copy of [`HubStats`].
#[derive(Debug, Clone, Default)]
pub struct StatsSnapshot {
    pub submitted:         u64,
    pub rejected:          u64,
    pub routed:            u64,
    pub unrouted:          u64,
    pub dropped:           u64,
    pub dispatch_failures: u64,
}
