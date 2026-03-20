use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
};
use orkester_plugin::sdk::Result;

/// Trait implemented by every logger backend.
pub trait Backend: Send {
    fn write(&mut self, line: &str) -> Result<()>;
}

// ── Console ───────────────────────────────────────────────────────────────────

/// Writes log lines to stdout.
pub struct ConsoleBackend;

impl ConsoleBackend {
    pub fn new() -> Self { Self }
}

impl Backend for ConsoleBackend {
    fn write(&mut self, line: &str) -> Result<()> {
        println!("{line}");
        Ok(())
    }
}

// ── File ──────────────────────────────────────────────────────────────────────

/// Appends log lines to a file, with a buffered writer for efficiency.
pub struct FileBackend {
    writer: BufWriter<File>,
}

impl FileBackend {
    pub fn open(path: &str) -> Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)
            .map_err(|e| format!("cannot open log file '{path}': {e}"))?;
        Ok(Self { writer: BufWriter::new(file) })
    }
}

impl Backend for FileBackend {
    fn write(&mut self, line: &str) -> Result<()> {
        writeln!(self.writer, "{line}")?;
        self.writer.flush()?;
        Ok(())
    }
}
