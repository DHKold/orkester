use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use anyhow::{Context, Result};
use serde_json::{json, Value};

use orkester_plugin::{
    abi::{AbiRequest, AbiResponse, AbiComponent},
    hub::{Envelope, builder::HubBuilder, config::HubConfig, ComponentRegistry, ComponentEntry},
    sdk::Host,
};

use crate::{catalog::Catalog, config::HostConfig};

fn extract_str(ptr: *const u8, len: u32) -> Option<String> {
    if ptr.is_null() || len == 0 {
        None
    } else {
        let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
        std::str::from_utf8(bytes).ok().map(|s| s.to_string())
    }
}

fn extract_bytes(ptr: *const u8, len: u32) -> Option<Vec<u8>> {
    if ptr.is_null() || len == 0 {
        None
    } else {
        let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
        Some(bytes.to_vec())
    }
}

fn make_error_response(id: u64, message: &str) -> AbiResponse {
    let body = json!({ "error": message });
    make_response(id, &body)
}

fn make_response(id: u64, body: &serde_json::Value) -> AbiResponse {
    let bytes = serde_json::to_vec(body).unwrap_or_default();
    let fmt = "std/json";
    let len = bytes.len() as u32;
    let ptr = Box::into_raw(bytes.into_boxed_slice()) as *mut u8;
    return AbiResponse {
        id:          id,
        format:      fmt.as_ptr(),
        format_len:  fmt.len() as u32,
        payload:     ptr,
        payload_len: len,
    };
}

fn decode_abi<T>(format: String, payload: Vec<u8>) -> Option<T> 
where T: serde::de::DeserializeOwned
{     
    match format.as_str() {
        "std/json" => serde_json::from_slice(&payload).ok(),
        "std/yaml" => serde_yaml::from_slice(&payload).ok(),
        "std/msgpack" => rmp_serde::from_slice(&payload).ok(),
        _ => None,
    }
}

fn make_routing_host(registry: ComponentRegistry, hub_config: HubConfig) -> Host {
    let rules = HubBuilder::new(hub_config, registry).build_rules();

    Host::with_callback(move |req: AbiRequest| -> AbiResponse {
        // 0. Extract raw request info for logging and routing
        let req_format: String = extract_str(req.format, req.format_len).unwrap_or("<invalid UTF-8>".to_string());
        log::debug!("[host/router] Received request id={} format='{}' payload_len={}", req.id, req_format, req.payload_len);
        let payload: Vec<u8> = extract_bytes(req.payload, req.payload_len).unwrap_or_default();

        // 1. Parse the request payload (supporting std/json, std/yaml, std/msgpack).
        //    If parsing fails, we log an error and let the envelope be None
        let envelope: Option<Envelope> = decode_abi(req_format, payload);

        // 2. If no envelope is found, return an error response
        if envelope.is_none() {
            log::error!("[host/router] Failed to parse request payload as JSON/YAML/MessagePack");
            return make_error_response(req.id, "The HUB was unable to parse request payload as JSON/YAML/MessagePack");
        }

        // 3. Find the first matching route based on the "kind" field in the envelope, and route the request accordingly. 
        // If no route matches, we log a warning and return an error response.
        let envelope = envelope.unwrap();
        let kind = &envelope.kind;
        log::debug!("[host/router] Routing request for kind '{}'", kind);

        let mut responses: Vec<Value> = Vec::new();
        for rule in &rules {
            if rule.matches(&envelope) {
                for dispatcher in &rule.dispatchers {
                    log::debug!("[host/router] Dispatching to '{}' for rule '{}'", dispatcher.name(), rule.name);
                    match dispatcher.dispatch(envelope.clone()) {
                        Ok(res) => responses.extend(res.into_iter().map(|e| {
                            log::debug!("[host/router] Dispatcher '{}' produced response envelope id={} kind='{}' format='{}' payload_len={}", dispatcher.name(), e.id, e.kind, e.format, e.payload.len());
                            let body = decode_abi(e.format.clone(), e.payload.clone()).unwrap_or(json!({"error": "failed to decode response envelope"}));
                            body
                        })),
                        Err(e) => log::warn!("[hub/router] rule '{}' dispatcher '{}': {e}", rule.name, dispatcher.name()),
                    }
                }
                break;
            }
        }
        make_response(req.id, &json!({ "status": "ok", "dispatched_to": responses.len(), "responses": responses }))
    })
}

// ── Main run loop ─────────────────────────────────────────────────────────────

/// Orchestrate the entire host lifecycle.
pub fn run(cfg: HostConfig) -> Result<()> {
    // 1. Create component registry, plugin roots, and routing host
    let registry    = Arc::new(Mutex::new(Vec::new()));
    let mut host    = make_routing_host(registry.clone(), cfg.hub.clone());

    // 2. Load plugins
    let mut catalog = Catalog::load(&mut host, &cfg.plugins).context("loading plugins")?;
    if catalog.components.is_empty() {
        log::warn!("[runner] no plugins loaded — running in demo mode");
    }

    // 3. Instantiate servers
    for server in &cfg.servers {
        match Catalog::instantiate_component(&mut catalog, server.kind.as_str(), &server.config) {
            Ok(component) => {
                register_component(&registry, &server.name, component, server.kind.clone());
                log::info!("[runner] Server '{}' of kind '{}' instantiated and registered", server.name, server.kind);
            }
            Err(e) => { log::error!("[runner] Failed to instantiate '{}': {e}", server.name); }
        }
    }

    // 4. Start servers based on their `start` confign (list of actions)
    for server in &cfg.servers {
        if server.start.is_empty() {
            continue;
        }
        let lock_registry = registry.lock().unwrap();
        let component_entry = match lock_registry.iter().find(|entry| entry.name == server.name) {
            Some(entry) => entry,
            None => {
                log::error!("[runner] Cannot start '{}': component not found in registry", server.name);
                continue;
            }
        };
        for action in &server.start {
            component_entry.call_json(&action.kind, action.config.clone());
            log::info!("[runner] Server '{}' start action '{}' dispatched", server.name, action.kind);
        }
    }

    // 5. Set up Ctrl+C shutdown
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        log::info!("[runner] Shutting down…");
        r.store(false, Ordering::SeqCst);
    }).context("setting Ctrl+C handler")?;

    // 6. Main loop — REST polling
    log::info!("[runner] Entering main loop (Ctrl+C to stop)");
    while running.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
    }

    // 7. Shutdown
    log::info!("[runner] stopped");
    Ok(())
}

pub fn register_component(registry: &ComponentRegistry, name: &str, component: *mut AbiComponent, kind: String) {
    let entry = ComponentEntry::new( name.to_string(), kind, component);
    let mut guard = registry.lock().unwrap();
    guard.push(entry);
}