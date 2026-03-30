use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::Mutex,
};

use serde::Deserialize;
use serde_json::Value;

use orkester_plugin::logging::LogRecord;
use super::LogSink;

#[derive(Debug, Deserialize)]
pub struct LocalFsSinkConfig {
    pub path:        String,
    #[serde(default = "default_max_size_mb")]
    pub max_size_mb: u64,
    #[serde(default = "default_max_files")]
    pub max_files:   u32,
}

fn default_max_size_mb() -> u64 { 100 }
fn default_max_files()   -> u32 { 5   }

pub struct LocalFsLogSink {
    config: LocalFsSinkConfig,
    file:   Mutex<File>,
}

impl LocalFsLogSink {
    pub fn from_config(value: &Value) -> Result<Self, String> {
        let cfg: LocalFsSinkConfig = serde_json::from_value(value.clone())
            .map_err(|e| e.to_string())?;
        let file = open_or_create(&cfg.path).map_err(|e| e.to_string())?;
        Ok(Self { config: cfg, file: Mutex::new(file) })
    }
}

impl LogSink for LocalFsLogSink {
    fn write(&self, _record: &LogRecord, formatted: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut file = self.file.lock().map_err(|_| "mutex poisoned")?;
        writeln!(file, "{formatted}")?;
        // Check size and rotate if needed.
        if let Ok(meta) = file.metadata() {
            if meta.len() >= self.config.max_size_mb * 1024 * 1024 {
                drop(file);  // release lock before rotate
                rotate(&self.config);
                *self.file.lock().map_err(|_| "mutex poisoned")? =
                    open_or_create(&self.config.path)?;
            }
        }
        Ok(())
    }
}

fn open_or_create(path: &str) -> std::io::Result<File> {
    if let Some(parent) = PathBuf::from(path).parent() {
        fs::create_dir_all(parent)?;
    }
    OpenOptions::new().create(true).append(true).open(path)
}

fn rotate(cfg: &LocalFsSinkConfig) {
    for i in (1..cfg.max_files).rev() {
        let src  = format!("{}.{}", cfg.path, i);
        let dest = format!("{}.{}", cfg.path, i + 1);
        if PathBuf::from(&src).exists() {
            let _ = fs::rename(&src, &dest);
        }
    }
    let rotated = format!("{}.1", cfg.path);
    let _ = fs::rename(&cfg.path, &rotated);
    // Prune oldest file.
    let oldest = format!("{}.{}", cfg.path, cfg.max_files + 1);
    let _ = fs::remove_file(&oldest);
}
