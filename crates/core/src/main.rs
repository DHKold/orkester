//! Entry point for the core app

mod config;
mod main_logging;
mod messaging;
mod plugin;
mod registry;
mod server;

use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use orkester_common::logging;

fn main() -> Result<(), Box<dyn Error>> {
    // Register a console consumer so logs are visible from the start.
    main_logging::init_logging();
    logging::Logger::info("Starting orkester...");

    // Parse CLI arguments
    let matches = clap::Command::new("orkester")
        .version("0.1.0")
        .about("Flexible orchestrationr for data workflows")
        .arg(
            clap::Arg::new("config")
                .short('c')
                .long("config-file")
                .num_args(1..)
                .help("Path to configuration file (can be used multiple times)"),
        )
        .arg(
            clap::Arg::new("set")
                .long("set")
                .num_args(1..)
                .help("Override config property: key=value"),
        )
        .get_matches();

    // Collect all config files
    let config_paths: Vec<String> = matches
        .get_many::<String>("config")
        .map(|vals| vals.cloned().collect())
        .unwrap_or_else(Vec::new);
    let overrides: Vec<String> = matches
        .get_many::<String>("set")
        .map(|vals| vals.cloned().collect())
        .unwrap_or_else(Vec::new);

    // Load configuration(s)
    let config_path_refs: Vec<&str> = config_paths.iter().map(|s| s.as_str()).collect();
    let override_refs: Vec<&str> = overrides.iter().map(|s| s.as_str()).collect();
    let config_tree = config::load_config_files(&config_path_refs, &override_refs);

    // Add any additional consumers based on config (e.g. file consumer)
    main_logging::load_logging_config(&config_tree);

    // Load plugins and register components/servers
    let plugins = plugin::load_plugins(&config_tree);
    let registry = registry::register_plugins(plugins);

    // Start servers as defined in config
    let (servers, hub_sides) = server::start_servers(&config_tree, &registry)?;

    // Build the message hub and register all server channels.
    let mut hub = messaging::Hub::new();
    for hub_side in hub_sides {
        hub.register(hub_side);
    }
    logging::Logger::info("Message hub ready.");

    // Setup graceful shutdown
    let running = Arc::new(AtomicBool::new(true));
    {
        let running = running.clone();
        ctrlc::set_handler(move || {
            logging::Logger::info("Shutdown signal received");
            running.store(false, Ordering::SeqCst);
        })?;
    }

    // Main loop: drive the message hub.
    while running.load(Ordering::SeqCst) {
        hub.poll();
        thread::sleep(Duration::from_millis(10));
    }

    logging::Logger::info("Shutting down orkester...");
    if let Err(e) = server::cleanup_servers(&servers) {
        logging::Logger::error(&format!("Error during server cleanup: {}", e));
    }
    Ok(())
}
