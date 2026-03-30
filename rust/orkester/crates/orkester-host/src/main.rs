use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

mod catalog;
mod config;
mod logging;
mod pipeline;
mod runner;
mod server;

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "orkester", about = "Orkester host — load and orchestrate components")]
struct Cli {
    /// Path to the host configuration file (YAML).
    #[arg(short, long, default_value = "config.yaml")]
    config: PathBuf,
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    let cli = Cli::parse();

    let yaml = std::fs::read_to_string(&cli.config)
        .with_context(|| format!("reading config file: {}", cli.config.display()))?;

    let cfg = config::HostConfig::from_yaml(&yaml)
        .with_context(|| format!("parsing config file: {}", cli.config.display()))?;

    runner::run(cfg)
}
