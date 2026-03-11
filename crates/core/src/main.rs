//! Entry point for the core app
//! Handles CLI parsing, logging setup, and main orchestration

mod config;
mod logging;
mod messaging;
mod plugin;
mod registry;
mod server;
mod types;

use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging early with default config (to capture all logs)
    let mut logging_handle = logging::init::init(&logging::LoggingConfig::default());
    tracing::info!("Starting orkester...");

    // Parse CLI arguments
    let matches = clap::Command::new("orkester")
        .version("0.1.0")
        .about("Flexible orchestrationr for data workflows")
        .arg(
            clap::Arg::new("config")
                .short('c')
                .long("config-file")
                .takes_value(true)
                .multiple_occurrences(true)
                .help("Path to configuration file (can be used multiple times)"),
        )
        .arg(
            clap::Arg::new("set")
                .long("set")
                .takes_value(true)
                .multiple_occurrences(true)
                .help("Override config property: key=value"),
        )
        .get_matches();

    // Collect all config files
    let config_paths: Vec<&str> = matches
        .values_of("config")
        .map_or(Vec::new(), |vals| vals.collect());
    let overrides: Vec<&str> = matches
        .values_of("set")
        .map_or(Vec::new(), |vals| vals.collect());

    // Load configuration(s)
    let config_tree = config::load_config_files(&config_paths, &overrides);

    // Update logging config if specified in config_tree
    if let Some(cfg) = config::extract_logging_config(&config_tree) {
        logging_handle.update(&cfg);
    }

    // Load plugins and register components/servers
    let plugins = plugin::load_plugins();
    registry::register_plugins(&plugins);

    // Start servers as defined in config
    let servers = server::start_servers(&config_tree);

    // Setup graceful shutdown
    let running = Arc::new(AtomicBool::new(true));
    {
        let running = running.clone();
        ctrlc::set_handler(move || {
            log::info!("Shutdown signal received");
            running.store(false, Ordering::SeqCst);
        })?;
    }

    // Monitor servers and handle inter-server communication
    while running.load(Ordering::SeqCst) {
        match messaging::monitor_and_handle(&servers) {
            Ok(_) => {},
            Err(e) => {
                log::error!("Server monitoring error: {}", e);
                // Optionally, trigger shutdown or escalate error
            }
        }
        thread::sleep(Duration::from_secs(1));
    }

    log::info!("Shutting down orkester...");
    if let Err(e) = server::cleanup_servers(&servers) {
        log::error!("Error during server cleanup: {}", e);
    }
    Ok(())
}
