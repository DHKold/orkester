mod catalog;
mod config;
mod hub;

use anyhow::Result;
use clap::Parser;
use hub::{Hub, Server};
use orkester_plugin::sdk::Host;
use serde_json::json;
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
    let mut hub = Hub::new(servers);
    eprintln!(
        "[host] hub ready — {} server(s), {} routed action(s)",
        hub.servers().len(),
        hub.route_count()
    );

    // ── 5. Run demo requests ─────────────────────────────────────────────────
    run_demo(&mut hub);

    // ── 6. Monitor — wait for shutdown signal ────────────────────────────────
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

    // ── 7. Graceful shutdown ─────────────────────────────────────────────────
    hub.shutdown();
    eprintln!("[host] goodbye");
    Ok(())
}

// ── Demo ──────────────────────────────────────────────────────────────────────

fn run_demo(hub: &mut Hub) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    eprintln!();
    eprintln!("  ┌──────────────────────────────────────────────────────┐");
    eprintln!("  │           Orkester Hub — Demo Requests               │");
    eprintln!("  └──────────────────────────────────────────────────────┘");

    // ── Logging ───────────────────────────────────────────────────────────────
    // The console-logger has min_level=warn, so only warn/error appear on stdout.
    // The audit-logger (if routed here) would capture everything from debug up.
    send(hub, "log info event",
        "sample/Log",
        json!({ "timestamp": ts, "level": "info", "message": "orkester-host demo started" }));
    send(hub, "log warning event",
        "sample/Log",
        json!({ "timestamp": ts + 1, "level": "warn", "message": "disk usage above 80%" }));
    send(hub, "log debug event (dropped — below min_level)",
        "sample/Log",
        json!({ "timestamp": ts + 2, "level": "debug", "message": "internal trace" }));

    // ── Arithmetic ────────────────────────────────────────────────────────────
    calc(hub, "add",  42.0,  8.0); //  50
    calc(hub, "mul",   7.0,  6.0); //  42
    calc(hub, "div", 100.0,  4.0); //  25
    calc(hub, "div",   1.0,  0.0); // error: division by zero

    // ── Counter ───────────────────────────────────────────────────────────────
    send(hub, "increment +10", "sample/Counter/Increment", json!({ "step": 10 }));
    send(hub, "increment +5",  "sample/Counter/Increment", json!({ "step":  5 }));
    send(hub, "decrement -3",  "sample/Counter/Decrement", json!({ "step":  3 }));
    match hub.route("sample/Counter/Get", json!(null)) {
        Ok(r)  => eprintln!("  ✓  counter value after +10 +5 -3  =  {}", r["value"]),
        Err(e) => eprintln!("  ✗  counter/get: {e}"),
    }

    // ── Echo / health probe ───────────────────────────────────────────────────
    match hub.route("sample/Echo", json!({ "message": "Hello, Orkester!" })) {
        Ok(r)  => eprintln!("  ✓  echo  →  {}", r["message"].as_str().unwrap_or("?")),
        Err(e) => eprintln!("  ✗  echo: {e}"),
    }

    eprintln!("  ──────────────────────────────────────────────────────");
    eprintln!();
}

fn send(hub: &mut Hub, label: &str, action: &str, params: serde_json::Value) {
    match hub.route(action, params) {
        Ok(_)  => eprintln!("  ✓  {label}"),
        Err(e) => eprintln!("  ✗  {label}: {e}"),
    }
}

fn calc(hub: &mut Hub, op: &str, a: f64, b: f64) {
    match hub.route("sample/Calculate", json!({ "op": op, "a": a, "b": b })) {
        Ok(r)  => eprintln!("  ✓  {a} {op} {b}  =  {}", r["result"]),
        Err(e) => eprintln!("  ✗  calculate({op}, {a}, {b}): {e}"),
    }
}
