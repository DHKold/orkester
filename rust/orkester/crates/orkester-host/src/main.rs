mod catalog;
mod config;
mod hub;

use anyhow::Result;
use clap::Parser;
use hub::{Hub, Server};
use orkester_plugin::sdk::Host;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name        = "orkester",
    about       = "Generic Orkester host — loads plugins and orchestrates components",
    version
)]
struct Cli {
    /// Configuration files to load (YAML / JSON / TOML).
    /// Multiple files are merged in the given order; later files override earlier ones.
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    configs: Vec<PathBuf>,

    /// Override a configuration key: KEY=VALUE.
    /// The value is parsed as JSON; bare strings are accepted without quotes.
    #[arg(long = "set", value_name = "KEY=VALUE")]
    overrides: Vec<String>,
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let cli = Cli::parse();

    // ── 1. Load & merge configuration ───────────────────────────────────────
    let cfg = config::load(&cli.configs, &cli.overrides)?;
    eprintln!("[host] configuration loaded ({} server(s) configured)", cfg.servers.len());

    // ── 2. Load plugins ──────────────────────────────────────────────────────
    let mut host = Host::new();
    let mut catalog = catalog::Catalog::load(&mut host, &cfg.plugins.directories)?;
    eprintln!("[host] plugins loaded");

    // ── 3. Instantiate configured servers ────────────────────────────────────
    let mut servers: Vec<Server> = Vec::new();
    for server_cfg in &cfg.servers {
        let name = server_cfg
            .name
            .clone()
            .unwrap_or_else(|| server_cfg.kind.clone());
        eprintln!("[host] starting server '{name}' ({})", server_cfg.kind);
        let component = catalog.create_component(&mut host, server_cfg)?;
        servers.push(Server::new(name, server_cfg.kind.clone(), component));
    }

    // ── 4. Build message hub ─────────────────────────────────────────────────
    let hub = Hub::new(servers);
    eprintln!("[host] hub ready with {} server(s)", hub.servers().len());

    // ── 5. Monitor — wait for shutdown signal ────────────────────────────────
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        eprintln!("\n[host] CTRL+C received — shutting down…");
        r.store(false, Ordering::SeqCst);
    })?;

    eprintln!("[host] running (press CTRL+C to stop)");
    while running.load(Ordering::SeqCst) {
        std::thread::sleep(std::time::Duration::from_millis(250));
    }

    // ── 6. Graceful shutdown ─────────────────────────────────────────────────
    hub.shutdown();
    eprintln!("[host] goodbye");
    Ok(())
}
