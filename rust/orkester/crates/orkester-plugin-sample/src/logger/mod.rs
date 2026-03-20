//! Structured logger with configurable output backends.
//!
//! # Backends
//! - [`ConsoleBackend`] — writes to stdout/stderr
//! - [`FileBackend`]    — appends to a file
//!
//! Multiple backends can be active simultaneously.

mod backend;
pub use backend::{Backend, ConsoleBackend, FileBackend};

use orkester_plugin::prelude::*;
use serde::{Deserialize, Serialize};

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Level::Debug => "DEBUG",
            Level::Info  => "INFO",
            Level::Warn  => "WARN",
            Level::Error => "ERROR",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Deserialize)]
pub struct LogRequest {
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
    pub level: Level,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct LogResponse {
    pub accepted: bool,
}

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LoggerConfig {
    /// Minimum log level to accept. Messages below this level are silently dropped.
    #[serde(default = "default_level")]
    pub min_level: Level,
    /// Backends to write to.
    #[serde(default)]
    pub backends: Vec<BackendConfig>,
}

fn default_level() -> Level { Level::Info }

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum BackendConfig {
    Console,
    File { path: String },
}

// ── Component ─────────────────────────────────────────────────────────────────

pub struct LoggerComponent {
    min_level: Level,
    backends: Vec<Box<dyn backend::Backend>>,
}

impl LoggerComponent {
    pub fn new(config: LoggerConfig) -> Result<Self> {
        let backends: Result<Vec<_>> = config
            .backends
            .into_iter()
            .map(|bc| -> Result<Box<dyn backend::Backend>> {
                match bc {
                    BackendConfig::Console => {
                        Ok(Box::new(ConsoleBackend::new()))
                    }
                    BackendConfig::File { path } => {
                        Ok(Box::new(FileBackend::open(&path)?))
                    }
                }
            })
            .collect();
        Ok(Self { min_level: config.min_level, backends: backends? })
    }
}

#[component(
    kind        = "sample/Logger:1.0",
    name        = "Logger",
    description = "Structured logger that fans out to one or more configurable backends."
)]
impl LoggerComponent {
    #[handle("sample/Log")]
    fn log(&mut self, req: LogRequest) -> Result<LogResponse> {
        if req.level < self.min_level {
            return Ok(LogResponse { accepted: false });
        }
        let line = format!("[{}] {} {}", req.timestamp, req.level, req.message);
        for backend in &mut self.backends {
            backend.write(&line)?;
        }
        Ok(LogResponse { accepted: true })
    }
}
