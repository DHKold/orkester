mod config;
mod plugins;

use std::path::PathBuf;
use clap::Parser;
use tracing::Level;

use config::AppConfig;
use plugins::load_plugins;

// ── CLI ────────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
#[command(
    name    = "orkester",
    about   = "Ultra-fast, resilient and secure workflow platform",
    version
)]
struct Cli {
    /// Path to a configuration file (JSON, YAML or TOML).
    #[arg(short = 'c', long = "config-file", value_name = "PATH")]
    config_file: Option<PathBuf>,
}

// ── Entry point ────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    // 1. Parse CLI arguments
    let cli = Cli::parse();
    tracing::debug!("CLI arguments parsed: config_file = {:?}", cli.config_file);

    // 2. Initialise console logging
    init_tracing();
    tracing::info!("Orkester starting up");

    // 3. Load configuration
    let config = load_config(&cli);
    tracing::info!(
        plugins_dir       = %config.plugins.dir.display(),
        plugins_recursive = config.plugins.recursive,
        "Configuration loaded"
    );

    // 4. Load plugins
    let plugins_dir = &config.plugins.dir;
    tracing::info!(dir = %plugins_dir.display(), recursive = config.plugins.recursive, "Scanning plugins directory");

    let (loaded, errors) = load_plugins(plugins_dir, config.plugins.recursive);

    tracing::info!(count = loaded.len(), "Plugins loaded successfully");
    if !errors.is_empty() {
        tracing::warn!(count = errors.len(), "Some plugins could not be loaded");
    }

    for plugin in &loaded {
        let meta = &plugin.plugin.metadata;
        tracing::info!(
            id          = %meta.id,
            name        = %meta.name,
            version     = %meta.version,
            components  = plugin.plugin.components.len(),
            "Registered plugin"
        );
    }

    // 5. TODO: start REST API, init engine, etc.
    tracing::info!("Orkester is ready");

    // 6. Shutdown
    tracing::info!("Orkester shutting down");
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .compact()
        .init();
}

fn load_config(cli: &Cli) -> AppConfig {
    match &cli.config_file {
        None => {
            tracing::info!("No configuration file specified, using defaults");
            AppConfig::default()
        }
        Some(path) => {
            tracing::info!(path = %path.display(), "Loading configuration file");
            match AppConfig::from_file(path) {
                Ok(cfg) => {
                    tracing::info!(path = %path.display(), "Configuration file loaded successfully");
                    cfg
                }
                Err(e) => {
                    // Config errors are fatal – log and exit cleanly.
                    eprintln!("ERROR: {e}");
                    std::process::exit(1);
                }
            }
        }
    }
}
