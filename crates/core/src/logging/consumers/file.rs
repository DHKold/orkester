use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::Path,
    sync::Mutex,
};

use crate::logging::consumer::LogConsumer;
use crate::logging::log::Log;

/// Appends each log entry as a plain-text line to a file.
///
/// The file is created if it does not exist, and appended to if it does.
/// Writes are flushed immediately so no entries are lost on crash.
///
/// # Example
/// ```no_run
/// use crate::logging::{Logger, consumers::FileConsumer};
///
/// Logger::add_consumer(FileConsumer::open("app.log").unwrap());
/// ```
pub struct FileConsumer {
    writer: Mutex<BufWriter<File>>,
}

impl FileConsumer {
    /// Opens (or creates) the file at `path` and returns a [`FileConsumer`]
    /// that appends to it.
    pub fn open(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self {
            writer: Mutex::new(BufWriter::new(file)),
        })
    }
}

impl LogConsumer for FileConsumer {
    fn consume(&self, log: &Log) {
        let tags = if log.tags.is_empty() {
            String::new()
        } else {
            format!("({}) ", log.tags.join(", "))
        };
        let line = format!(
            "[{}] {:<5} [{}] {}{}\n",
            log.datetime.format("%Y-%m-%dT%H:%M:%S%.3fZ"),
            log.level.to_string(),
            log.source,
            tags,
            log.message,
        );
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writer.write_all(line.as_bytes());
            let _ = writer.flush();
        }
    }
}
