use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use anyhow::{Context, Result};
use serde_json::{json, Value};

use orkester_plugin::{
    log_error, log_warn, log_info, log_debug,    
    abi::AbiComponent,
    sdk::PluginComponent,
    hub::{Envelope, builder::HubBuilder, config::HubConfig, ComponentRegistry, ComponentEntry},
};

use crate::{
    catalog::Catalog,
    config::HostConfig,
    logging::{HostLogBridge, LOGGING_SERVER_KIND_PREFIX},
    pipeline::{make_pipeline_host, HostRequest, HostResponse},
    server::{ComponentInfo, ComponentInfoRegistry, HostServer},
};

// ─── Encoding helpers ─────────────────────────────────────────────────────────

/// Deserializes a pipeline request's payload into `T`.
///
/// Strips any `+modifier` suffix from the format string (e.g. `"+fire"`,
/// `"+async"`) before selecting the codec, so the same routing logic handles
/// all modes.
fn decode_request<T>(req: &HostRequest) -> Option<T>
where
    T: serde::de::DeserializeOwned,
{
    // Strip the mode modifier (everything after the first '+') to get the
    // base serialization format.
    let base = req.format.split('+').next().unwrap_or(&req.format);
    match base {
        "std/json"    => serde_json::from_slice(&req.payload).ok(),
        "std/yaml"    => serde_yaml::from_slice(&req.payload).ok(),
        "std/msgpack" => rmp_serde::from_slice(&req.payload).ok(),
        // Unknown or empty format — attempt JSON as a best-effort fallback.
        _             => serde_json::from_slice(&req.payload).ok(),
    }
}

/// Serializes `body` as a JSON `HostResponse` for `request_id`.
fn json_response(request_id: u64, body: &Value) -> HostResponse {
    HostResponse {
        request_id,
        payload: serde_json::to_vec(body).unwrap_or_default(),
    }
}

/// Produces an error `HostResponse`.
fn error_response(request_id: u64, message: &str) -> HostResponse {
    json_response(request_id, &json!({ "error": message }))
}

// ─── Routing host factory ─────────────────────────────────────────────────────

/// Creates a `Host` backed by the async pipeline.
///
/// The routing closure (running on the worker thread) parses each inbound
/// `HostRequest`, matches it against the hub rules, dispatches to the target
/// components, and returns a `HostResponse`.  All I/O with components is still
/// synchronous from the worker's perspective; the pipeline's value here is that
/// the ABI ingress never blocks on routing logic.
///
/// Requests with a `log/*` format are intercepted before hub routing and
/// forwarded directly to the `HostLogBridge` (dedicated logging path).
fn make_routing_host(
    registry: ComponentRegistry,
    hub_config: HubConfig,
    log_bridge: Arc<HostLogBridge>,
) -> orkester_plugin::sdk::Host {
    let rules = HubBuilder::new(hub_config, registry).build_rules();

    make_pipeline_host(
        move |req: HostRequest| -> Option<HostResponse> {
            let id = req.id;

            // Dedicated logging path — bypass hub routing entirely.
            if req.format.starts_with("log/") {
                log_bridge.submit(&req.payload);
                return Some(HostResponse { request_id: id, payload: Vec::new() });
            }

            // Parse the inbound envelope.
            let envelope: Option<Envelope> = decode_request(&req);
            if envelope.is_none() {
                log_error!("[host/worker] failed to parse request {} as Envelope", id);
                return Some(error_response(id, "failed to parse request payload as Envelope"));
            }

            let envelope = envelope.unwrap();
            log_debug!("[host/worker] routing request {} kind='{}'", id, envelope.kind);

            // Match and dispatch through hub rules.
            let mut dispatched = 0usize;
            let mut responses:  Vec<Value> = Vec::new();

            for rule in &rules {
                if !rule.matches(&envelope) {
                    continue;
                }
                for dispatcher in &rule.dispatchers {
                    log_debug!(
                        "[host/worker] dispatching req {} to '{}' (rule '{}')",
                        id, dispatcher.name(), rule.name
                    );
                    match dispatcher.dispatch(envelope.clone()) {
                        Ok(envs) => {
                            dispatched += envs.len();
                            responses.extend(envs.into_iter().map(|e| {
                                serde_json::from_slice(&e.payload)
                                    .unwrap_or(json!({ "error": "failed to decode response" }))
                            }));
                        }
                        Err(e) => {
                            log_warn!("[host/worker] rule '{}' dispatcher '{}': {}", rule.name, dispatcher.name(), e);
                        }
                    }
                }
                break; // first matching rule wins
            }

            Some(json_response(
                id,
                &json!({ "status": "ok", "dispatched_to": dispatched, "responses": responses }),
            ))
        },
        HashMap::new(), // no async callbacks registered yet
    )
}

// ── Main run loop ─────────────────────────────────────────────────────────────

/// Orchestrate the entire host lifecycle.
pub fn run(cfg: HostConfig) -> Result<()> {
    // 1. Create component registry, logging bridge, host and internal server.
    let registry: Arc<Mutex<Vec<ComponentEntry>>> = Arc::new(Mutex::new(Vec::new()));
    let info_registry: ComponentInfoRegistry = Arc::new(Mutex::new(Vec::new()));
    let log_bridge = HostLogBridge::new();
    let mut host = make_routing_host(registry.clone(), cfg.hub.clone(), log_bridge.clone());
    // 2. Load plugins
    let mut catalog = Catalog::load(&mut host, &cfg.plugins).context("loading plugins")?;
    if catalog.components.is_empty() {
        log_warn!("[runner] no plugins loaded — running in demo mode");
    }

    // 3. Instantiate servers
    for server in &cfg.servers {
        match Catalog::instantiate_component(&mut catalog, server.kind.as_str(), &server.config) {
            Ok(component) => {
                // Connect the logging bridge before generic registration so the
                // server is fully registered even if bridge connection fails.
                if server.kind.starts_with(LOGGING_SERVER_KIND_PREFIX) {
                    let entry = ComponentEntry::new(server.name.clone(), server.kind.clone(), component);
                    log_bridge.connect(entry);
                    log_info!("[runner] Logging bridge connected to '{}'", server.name);
                }
                register_component(&registry, &info_registry, &server.name, component, server.kind.clone());
                log_info!("[runner] Server '{}' of kind '{}' instantiated and registered", server.name, server.kind);
            }
            Err(e) => { log_error!("[runner] Failed to instantiate '{}': {e}", server.name); }
        }
    }

    // 3.5. Register the internal host server for introspection and plugin management.
    let host_server = HostServer::new(catalog, info_registry.clone());
    register_component(&registry, &info_registry, "host-server", &mut host_server.to_abi(), "orkester/HostServer:1.0".to_string());

    // 4. Start servers based on their `start` confign (list of actions)
    for server in &cfg.servers {
        if server.start.is_empty() {
            continue;
        }
        let lock_registry = registry.lock().unwrap();
        let component_entry = match lock_registry.iter().find(|entry| entry.name == server.name) {
            Some(entry) => entry,
            None => {
                log_error!("[runner] Cannot start '{}': component not found in registry", server.name);
                continue;
            }
        };
        for action in &server.start {
            let _ = component_entry.call_json(&action.kind, action.config.clone());
            log_info!("[runner] Server '{}' start action '{}' dispatched", server.name, action.kind);
        }
    }

    // 5. Set up Ctrl+C shutdown
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        log_info!("[runner] Shutting down…");
        r.store(false, Ordering::SeqCst);
    }).context("setting Ctrl+C handler")?;

    // 6. Main loop — REST polling
    log_info!("[runner] Entering main loop (Ctrl+C to stop)");
    while running.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
    }

    // 7. Shutdown
    log_info!("[runner] stopped");
    Ok(())
}

pub fn register_component(registry: &ComponentRegistry, info_registry: &ComponentInfoRegistry, name: &str, component: *mut AbiComponent, kind: String) {
    let entry = ComponentEntry::new(name.to_string(), kind.clone(), component);
    registry.lock().unwrap().push(entry);
    info_registry.lock().unwrap().push(ComponentInfo { name: name.to_string(), kind });
}