mod handler;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use orkester_plugin::sdk::{Plugin, Request};

use handler::LoggingHostHandler;

// ─── CLI ──────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name    = "orkester",
    version,
    about   = "Orkester – ultra-fast, resilient and secure workflow platform",
    long_about = None,
)]
struct Cli {
    /// Path to the plugin shared library (.so / .dll / .dylib).
    #[arg(short, long, global = true, value_name = "PATH")]
    plugin: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Send a Ping to the plugin and print the response.
    Ping,

    /// Print the plugin's component catalogue as JSON.
    Metadata,

    /// Create a component of the given kind and send a JSON request to it.
    ///
    /// The response is printed as pretty-printed JSON.
    ///
    /// Example:
    ///   orkester --plugin ./libfoo.so run --kind Counter --payload '{"type":"Inc"}'
    Run {
        /// Component kind name (e.g. Echo, Counter, Calculator, Greeter).
        #[arg(short, long, value_name = "KIND")]
        kind: String,

        /// Request payload as a JSON string.
        #[arg(short = 'd', long, value_name = "JSON")]
        payload: String,
    },
}

// ─── Entry point ──────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let cli = Cli::parse();

    // All subcommands that interact with a plugin require --plugin.
    let plugin_path = cli
        .plugin
        .context("--plugin <PATH> is required for this command")?;

    // Load the plugin; host callbacks are printed to stderr.
    let plugin = Plugin::load_with_handler(&plugin_path, LoggingHostHandler)
        .with_context(|| format!("failed to load plugin '{}'", plugin_path.display()))?;

    match cli.command {
        // ── Ping ──────────────────────────────────────────────────────────────
        Command::Ping => {
            let req = Request::json(1, &serde_json::json!({ "type": "Ping" }))?;
            let resp = plugin.root().handle(req);
            let text = resp.as_str()?;
            println!("{text}");
        }

        // ── Metadata ──────────────────────────────────────────────────────────
        Command::Metadata => {
            let req = Request::json(1, &serde_json::json!({ "type": "Metadata" }))?;
            let resp = plugin.root().handle(req);
            let value: serde_json::Value = resp.as_json()?;
            println!("{}", serde_json::to_string_pretty(&value)?);
        }

        // ── Run ───────────────────────────────────────────────────────────────
        Command::Run { kind, payload } => {
            // Step 1: ask the root to create the requested component kind.
            let create_req = Request::json(
                1,
                &serde_json::json!({ "type": "CreateComponent", "kind": kind }),
            )?;
            let component = plugin
                .root()
                .create_component(create_req)
                .with_context(|| format!("failed to create component of kind '{kind}'"))?;

            // Step 2: parse the user's payload JSON and forward it.
            let parsed: serde_json::Value =
                serde_json::from_str(&payload).context("--payload is not valid JSON")?;
            let req = Request::json(2, &parsed)?;
            let resp = component.handle(req);

            // Pretty-print whatever the component returns.
            let value: serde_json::Value = resp.as_json()?;
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
    }

    Ok(())
}
