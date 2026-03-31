use std::sync::Mutex;

use orkester_plugin::prelude::*;

use serde::Deserialize;
use serde_json::Value;

use orkester_plugin::logging::LogRecord;
use super::LogSink;

#[derive(Debug, Deserialize)]
pub struct S3SinkConfig {
    pub bucket:       String,
    pub prefix:       String,
    pub region:       String,
    #[serde(default = "default_buffer_mb")]
    pub buffer_mb:    u64,
    #[serde(default = "default_flush_secs")]
    pub flush_secs:   u64,
}

fn default_buffer_mb()  -> u64 { 10 }
fn default_flush_secs() -> u64 { 60 }

/// S3LogSink buffers records locally and uploads batches on rotation.
///
/// # Note
/// Full S3 upload requires AWS credentials at runtime.  The sink will log a
/// warning and retain records locally until credentials are supplied.
pub struct S3LogSink {
    config: S3SinkConfig,
    buffer: Mutex<Vec<String>>,
}

impl S3LogSink {
    pub fn from_config(value: &Value) -> Result<Self, String> {
        let cfg: S3SinkConfig = serde_json::from_value(value.clone())
            .map_err(|e| e.to_string())?;
        log_warn!("[logging/S3LogSink] S3 upload is not yet implemented; records are buffered in memory only.");
        Ok(Self { config: cfg, buffer: Mutex::new(Vec::new()) })
    }
}

impl LogSink for S3LogSink {
    fn write(&self, _record: &LogRecord, formatted: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = self.buffer.lock().map_err(|_| "mutex poisoned")?;
        buf.push(formatted.to_owned());
        // TODO: flush to s3://{bucket}/{prefix} when buffer exceeds buffer_mb
        Ok(())
    }
}
