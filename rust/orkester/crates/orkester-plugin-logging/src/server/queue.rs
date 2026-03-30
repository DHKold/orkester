use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use orkester_plugin::logging::LogRecord;

/// Thin wrapper over a bounded crossbeam channel.
pub struct LogQueue {
    sender: Sender<LogRecord>,
}

impl LogQueue {
    /// Returns `(queue, receiver)` — the receiver is moved into the worker.
    pub fn new(capacity: usize) -> (Self, Receiver<LogRecord>) {
        let (sender, receiver) = bounded(capacity);
        (Self { sender }, receiver)
    }

    /// Non-blocking send.  Returns `Err` if the queue is full.
    pub fn try_send(&self, record: LogRecord) -> Result<(), TrySendError<LogRecord>> {
        self.sender.try_send(record)
    }
}
