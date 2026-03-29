//! Control channel events for the Cron scheduler.

use workaholic::CronDoc;

/// Events sent over the control channel to the scheduler background thread.
#[derive(Debug, Clone)]
pub enum CronControlEvent {
    /// Add or overwrite a Cron entry.
    Register(CronDoc),
    /// Remove the Cron with the given name.
    Unregister { name: String },
    /// Shut down the scheduler gracefully.
    Shutdown,
}

/// Sender half of the control channel.
pub struct CronControl {
    tx: crossbeam_channel::Sender<CronControlEvent>,
}

impl CronControl {
    pub fn new(tx: crossbeam_channel::Sender<CronControlEvent>) -> Self {
        Self { tx }
    }

    pub fn register(&self, cron: CronDoc) {
        let _ = self.tx.send(CronControlEvent::Register(cron));
    }

    pub fn unregister(&self, name: impl Into<String>) {
        let _ = self.tx.send(CronControlEvent::Unregister { name: name.into() });
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(CronControlEvent::Shutdown);
    }
}
