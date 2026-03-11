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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::{consumer::LogConsumer, level::Level, log::Log};

    fn make_log(msg: &str) -> Log {
        Log::new(Level::INFO, "test", vec![], msg)
    }

    #[test]
    fn writes_entry_to_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.log");
        let consumer = FileConsumer::open(&path).unwrap();
        consumer.consume(&make_log("hello from file"));
        drop(consumer);

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("hello from file"), "missing message: {content}");
        assert!(content.contains("INFO"), "missing level: {content}");
        assert!(content.contains("[test]"), "missing source: {content}");
    }

    #[test]
    fn appends_multiple_entries_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.log");
        let consumer = FileConsumer::open(&path).unwrap();
        consumer.consume(&make_log("first"));
        consumer.consume(&make_log("second"));
        drop(consumer);

        let content = std::fs::read_to_string(&path).unwrap();
        let first_pos = content.find("first").unwrap();
        let second_pos = content.find("second").unwrap();
        assert!(first_pos < second_pos, "entries out of order");
    }

    #[test]
    fn reopening_file_appends_not_truncates() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.log");

        let c1 = FileConsumer::open(&path).unwrap();
        c1.consume(&make_log("before reopen"));
        drop(c1);

        let c2 = FileConsumer::open(&path).unwrap();
        c2.consume(&make_log("after reopen"));
        drop(c2);

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("before reopen"));
        assert!(content.contains("after reopen"));
    }

    #[test]
    fn tags_appear_in_output() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.log");
        let consumer = FileConsumer::open(&path).unwrap();
        let log = Log::new(Level::WARN, "svc", vec!["tag-a".into(), "tag-b".into()], "tagged");
        consumer.consume(&log);
        drop(consumer);

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("tag-a"), "missing tag: {content}");
        assert!(content.contains("tag-b"), "missing tag: {content}");
        assert!(content.contains("tagged"), "missing message: {content}");
    }

    #[test]
    fn entry_is_newline_terminated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.log");
        let consumer = FileConsumer::open(&path).unwrap();
        consumer.consume(&make_log("newline check"));
        drop(consumer);

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.ends_with('\n'), "file should end with newline");
    }
}
