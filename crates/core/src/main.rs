mod config;
mod plugins;
mod servers;

use std::path::PathBuf;
use clap::Parser;
use tracing::Level;

use config::AppConfig;
use plugins::load_plugins;
use servers::build_servers;

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

    // 2. Initialise console logging
    init_tracing();
    tracing::info!("Orkester starting up");

    // 3. Load configuration
    let config = load_config(&cli);
    tracing::info!(
        plugins_dir       = config.get_str("plugins.dir", "./plugins"),
        plugins_recursive = config.get_bool("plugins.recursive", false),
        state_plugin      = config.get_str("servers.state.plugin", "basic-state-server"),
        workflow_plugin   = config.get_str("servers.workflow.plugin", "basic-workflow-server"),
        metrics_plugin    = config.get_str("servers.metrics.plugin", "no-metrics-server"),
        rest_plugin       = config.get_str("servers.rest.plugin", "axum-rest-server"),
        "Configuration loaded"
    );

    // 4. Load plugins
    let plugins_dir = config.get_str("plugins.dir", "./plugins");
    let plugins_recursive = config.get_bool("plugins.recursive", false);
    tracing::info!(
        dir       = plugins_dir,
        recursive = plugins_recursive,
        "Scanning plugins directory"
    );

    let (mut loaded, errors) = load_plugins(
        std::path::Path::new(plugins_dir),
        plugins_recursive,
    );

    if !errors.is_empty() {
        tracing::warn!(count = errors.len(), "Some plugins could not be loaded");
    }

    for plugin in &loaded {
        let meta = &plugin.plugin.metadata;
        tracing::info!(
            id         = %meta.id,
            name       = %meta.name,
            version    = %meta.version,
            components = plugin.plugin.components.len(),
            "Registered plugin"
        );
    }
    tracing::info!(count = loaded.len(), "Plugin loading complete");

    // 5. Build servers (drains factories from loaded plugins; libraries stay alive)
    let built = match build_servers(config.value(), &mut loaded) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = %e, "Failed to build servers — aborting");
            std::process::exit(1);
        }
    };

    let rest_bind = built.rest.handle().bound_addr().to_string();
    tracing::info!(
        state    = built.state.name(),
        workflow = built.workflow.name(),
        metrics  = built.metrics.name(),
        rest     = %rest_bind,
        "All servers built — starting"
    );

    // 7. Run all servers concurrently; each runs until shutdown
    tokio::join!(
        built.state.run(),
        built.workflow.run(),
        built.metrics.run(),
        built.rest.run(),
    );

    // `loaded` is dropped here — after all servers have stopped
    drop(loaded);
    tracing::info!("Orkester shut down cleanly");
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
