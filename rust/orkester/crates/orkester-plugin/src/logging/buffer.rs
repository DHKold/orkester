use std::collections::VecDeque;
use super::record::LogRecord;

/// Bounded ring buffer. When full the *oldest* record is dropped.
pub struct LogBuffer {
    records:  VecDeque<LogRecord>,
    capacity: usize,
    /// Number of records that were silently dropped due to overflow.
    pub dropped: u64,
}

impl LogBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            records: VecDeque::with_capacity(capacity),
            capacity,
            dropped: 0,
        }
    }

    pub fn push(&mut self, record: LogRecord) {
        if self.records.len() >= self.capacity {
            self.records.pop_front();
            self.dropped += 1;
        }
        self.records.push_back(record);
    }

    pub fn drain(&mut self) -> impl Iterator<Item = LogRecord> + '_ {
        self.records.drain(..)
    }
}

impl Default for LogBuffer {
    fn default() -> Self { Self::new(256) }
}
