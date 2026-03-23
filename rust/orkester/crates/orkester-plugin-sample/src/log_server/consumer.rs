use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write as _},
    path::PathBuf,
};

// ── LogConsumer trait ─────────────────────────────────────────────────────────

pub trait LogConsumer: Send {
    /// Consume a pre-formatted log line.
    fn consume(&mut self, line: &str);
}

// ── Console consumer ──────────────────────────────────────────────────────────

pub struct ConsoleConsumer;

impl LogConsumer for ConsoleConsumer {
    fn consume(&mut self, line: &str) {
        println!("{line}");
    }
}

// ── File consumer ─────────────────────────────────────────────────────────────

pub struct FileConsumer {
    writer: BufWriter<File>,
}

impl FileConsumer {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, String> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path.into())
            .map_err(|e| e.to_string())?;
        Ok(Self { writer: BufWriter::new(file) })
    }
}

impl LogConsumer for FileConsumer {
    fn consume(&mut self, line: &str) {
        let _ = writeln!(self.writer, "{line}");
        let _ = self.writer.flush();
    }
}
