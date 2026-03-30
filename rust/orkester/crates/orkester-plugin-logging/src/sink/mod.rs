pub mod console;
pub mod local_fs;
pub mod s3;

use orkester_plugin::logging::LogRecord;

/// Writes a formatted log line to an output destination.
pub trait LogSink: Send + 'static {
    fn write(&self, record: &LogRecord, formatted: &str) -> Result<(), Box<dyn std::error::Error>>;
}

pub type BoxedSink = Box<dyn LogSink>;
