//! # Hub
//!
//! The hub is an in-process, configuration-driven message-routing layer that
//! runs on the **host** side.  It mediates every interaction between components
//! that is not a direct ABI call:
//!
//! ```text
//! host code / AbiHost.handle()
//!       │
//!       ▼
//!   MessageHub::submit(Envelope)
//!       │
//!       ▼
//!   Router thread  ── evaluates route rules ──▶  Dispatcher ──▶ ABI Component
//! ```
//!
//! ## Quick-start (host side)
//!
//! ```ignore
//! use orkester_plugin::hub::{MessageHub, ComponentRegistry, ComponentEntry};
//! use orkester_plugin::hub::config::HubConfig;
//!
//! let registry: ComponentRegistry = Default::default();
//! let hub = MessageHub::new(config, registry.clone())?;
//! hub.start()?;
//!
//! hub.submit(Envelope::from_json(next_id(), None, "log/Entry", json!({…})))?;
//! hub.stop()?;
//! ```

pub mod builder;
pub mod config;
pub mod dispatcher;
pub mod envelope;
pub mod error;
pub mod filter;
pub mod router;
pub mod stats;

pub use dispatcher::{ComponentEntry, ComponentRegistry};
pub use envelope::Envelope;
pub use error::{HubError, SubmitError};
pub use stats::{HubStats, StatsSnapshot};

use std::sync::Arc;
use std::thread::JoinHandle;

use self::config::{BackpressurePolicy, HubConfig};

use self::builder::HubBuilder;

// ── MessageHub ────────────────────────────────────────────────────────────────

/// Manages the lifecycle of the hub router thread and exposes the `submit` API.
pub struct MessageHub {
    config:        HubConfig,
    registry:      ComponentRegistry,
    stats:         Arc<HubStats>,
    tx:            Option<crossbeam_channel::Sender<Envelope>>,
    router_thread: Option<JoinHandle<()>>,
}

impl MessageHub {
    // ── Lifecycle ─────────────────────────────────────────────────────────

    /// Create a new hub and validate the configuration.
    ///
    /// Does **not** start any background threads.  Call [`start`](Self::start)
    /// when component registration is complete.
    ///
    /// `registry` must be populated (or at least initialised) before `start` is
    /// called; it may continue to be modified afterwards — dispatchers read it
    /// under a lock on every delivery.
    pub fn new(config: HubConfig, registry: ComponentRegistry) -> Result<Self, HubError> {
        HubBuilder::new(config.clone(), registry.clone()).validate()?;
        Ok(Self {
            config,
            registry,
            stats: Arc::new(HubStats::default()),
            tx: None,
            router_thread: None,
        })
    }

    /// Start the router background thread.
    ///
    /// Returns `AlreadyRunning` if the hub is already active.
    pub fn start(&mut self) -> Result<(), HubError> {
        if self.tx.is_some() {
            return Err(HubError::AlreadyRunning);
        }

        let capacity = self.config.queue.waiting_capacity;
        let (tx, rx) = crossbeam_channel::bounded(capacity);
        let builder = HubBuilder::new(self.config.clone(), self.registry.clone());
        let router  = builder.build_router(rx, self.stats.clone())?;

        let handle = std::thread::Builder::new()
            .name("hub-router".to_owned())
            .spawn(move || router.run())
            .map_err(|e| HubError::WorkerFailed(e.to_string()))?;

        self.tx            = Some(tx);
        self.router_thread = Some(handle);
        Ok(())
    }

    /// Stop the hub by closing the input channel and joining the router thread.
    ///
    /// All envelopes already in the queue will be drained before the thread
    /// exits.  Any further `submit` calls will return [`SubmitError::NotRunning`].
    pub fn stop(&mut self) -> Result<(), HubError> {
        // Dropping the sender closes the channel; the router's iterator exits.
        drop(self.tx.take());
        if let Some(handle) = self.router_thread.take() {
            handle
                .join()
                .map_err(|_| HubError::WorkerFailed("router thread panicked".to_owned()))?;
        }
        Ok(())
    }

    // ── Submission ────────────────────────────────────────────────────────

    /// Submit an envelope to the hub, returning immediately.
    ///
    /// Backpressure behaviour is determined by [`BackpressurePolicy`] in
    /// the queue config.
    pub fn submit(&self, envelope: Envelope) -> Result<(), SubmitError> {
        let tx = self.tx.as_ref().ok_or(SubmitError::NotRunning)?;

        match self.config.queue.backpressure {
            BackpressurePolicy::Block => tx
                .send(envelope)
                .map_err(|_| SubmitError::NotRunning),

            BackpressurePolicy::Reject => tx
                .try_send(envelope)
                .map_err(|e| {
                    self.stats.inc_rejected();
                    match e {
                        crossbeam_channel::TrySendError::Full(_)         => SubmitError::QueueFull,
                        crossbeam_channel::TrySendError::Disconnected(_) => SubmitError::NotRunning,
                    }
                }),

            // Best-effort: drop the incoming message if the queue is full.
            BackpressurePolicy::DropNewest | BackpressurePolicy::DropOldest => {
                let _ = tx.try_send(envelope);
                Ok(())
            }
        }
    }

    // ── Introspection ─────────────────────────────────────────────────────

    pub fn is_running(&self) -> bool { self.tx.is_some() }

    pub fn stats(&self) -> &HubStats { &self.stats }

    pub fn stats_snapshot(&self) -> StatsSnapshot { self.stats.snapshot() }
}

impl Drop for MessageHub {
    fn drop(&mut self) {
        // Best-effort graceful shutdown if not already stopped.
        drop(self.tx.take());
        if let Some(handle) = self.router_thread.take() {
            let _ = handle.join();
        }
    }
}
