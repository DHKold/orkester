use std::sync::Arc;

use crossbeam_channel::Receiver;

use crate::hub::{envelope::Envelope, filter::Filter, stats::HubStats};

use super::dispatcher::Dispatcher;

// ── RouteRule ─────────────────────────────────────────────────────────────────

pub struct RouteRule {
    pub name: String,
    /// A message matches if **any** filter returns `true`.
    pub filters: Vec<Box<dyn Filter>>,
    /// On match the message is delivered to **all** dispatchers.
    pub dispatchers: Vec<Box<dyn Dispatcher>>,
}

impl RouteRule {
    fn matches(&self, envelope: &Envelope) -> bool {
        self.filters.iter().any(|f| f.matches(envelope))
    }
}

// ── Router ────────────────────────────────────────────────────────────────────

/// Blocking router that runs on a dedicated background thread.
///
/// The thread exits naturally when the associated `Sender` (in `MessageHub`)
/// is dropped, which closes the channel and unblocks the iterator.
pub struct Router {
    rx:    Receiver<Envelope>,
    rules: Vec<RouteRule>,
    stats: Arc<HubStats>,
}

impl Router {
    pub fn new(rx: Receiver<Envelope>, rules: Vec<RouteRule>, stats: Arc<HubStats>) -> Self {
        Self { rx, rules, stats }
    }

    /// Blocking event loop — call from a dedicated thread.
    pub fn run(self) {
        for envelope in &self.rx {
            self.stats.inc_submitted();
            let mut matched_any = false;

            for rule in &self.rules {
                if rule.matches(&envelope) {
                    matched_any = true;
                    for dispatcher in &rule.dispatchers {
                        if let Err(e) = dispatcher.dispatch(envelope.clone()) {
                            log::warn!("[hub/router] rule '{}' dispatcher '{}': {e}",
                                rule.name, dispatcher.name());
                            self.stats.inc_dispatch_failures();
                        }
                    }
                }
            }

            if matched_any {
                self.stats.inc_routed();
            } else {
                self.stats.inc_unrouted();
                log::debug!(
                    "[hub/router] id={} kind='{}' matched no rule — unrouted",
                    envelope.id, envelope.kind
                );
            }
        }
        log::debug!("[hub/router] channel closed — router thread exiting");
    }
}
