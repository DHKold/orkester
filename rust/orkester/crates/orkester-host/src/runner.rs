use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{Context, Result};
use serde_json::{json, Value};

use orkester_plugin::{
    abi::{AbiComponent, AbiRequest, AbiResponse},
    hub::{Envelope, MessageHub},
    sdk::Host,
};

use crate::{
    catalog::Catalog,
    config::HostConfig,
    registry::{self, ComponentRegistry},
};

// ── Monotonic ID ──────────────────────────────────────────────────────────────

static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

// ── ABI call helpers ──────────────────────────────────────────────────────────

/// Call a raw `*mut AbiComponent` with a JSON envelope, returning the parsed
/// JSON response body (or an error).
///
/// # Safety
/// `ptr` must be a valid, live ABI component pointer.
pub fn call_json(ptr: *mut AbiComponent, action: &str, params: Value) -> Result<Value> {
    // Normalize null → {} so handlers that take a struct with all-default
    // fields (e.g. PingRequest) can deserialize it without error.
    let params = if params.is_null() { json!({}) } else { params };
    let body = json!({ "action": action, "params": params });
    let body_bytes = serde_json::to_vec(&body)?;
    let fmt = "std/json";

    let req = AbiRequest {
        id:          0,
        format:      fmt.as_ptr(),
        format_len:  fmt.len() as u32,
        payload:     body_bytes.as_ptr(),
        payload_len: body_bytes.len() as u32,
    };

    unsafe {
        let res = ((*ptr).handle)(ptr, req);
        let payload = if res.payload.is_null() || res.payload_len == 0 {
            &[] as &[u8]
        } else {
            std::slice::from_raw_parts(res.payload, res.payload_len as usize)
        };
        let v: Value = serde_json::from_slice(payload).unwrap_or(Value::Null);
        ((*ptr).free_response)(ptr, res);
        Ok(v)
    }
}

// ── Host routing callback ─────────────────────────────────────────────────────

/// Build a `Host::with_callback` closure that routes calls through the
/// component registry.
///
/// When a plugin component (e.g. `RestServer`) calls `host.handle(...)`, the
/// request JSON envelope's `action` field is used to find the first registered
/// component whose `kind` starts with the namespace (e.g. `"ping"` matches
/// `"sample/PingServer:1.0"`), and the request is forwarded to it.
fn make_routing_host(registry: ComponentRegistry) -> Host {
    Host::with_callback(move |req: AbiRequest| -> AbiResponse {
        // Decode the incoming JSON envelope
        let payload = unsafe {
            if req.payload.is_null() || req.payload_len == 0 {
                &[] as &[u8]
            } else {
                std::slice::from_raw_parts(req.payload, req.payload_len as usize)
            }
        };

        let envelope: Value = serde_json::from_slice(payload).unwrap_or(Value::Null);
        let action = envelope["action"].as_str().unwrap_or("");
        // Save id before req is potentially moved into handle()
        let req_id = req.id;

        // Find a component that can handle this action
        let target_ptr: Option<*mut AbiComponent> = {
            let guard = registry.lock().unwrap();
            guard.iter().find(|e| {
                // Heuristic: action namespace = first component of kind after "/"
                // e.g. action "ping/Ping" → look for kind starting with "sample/PingServer"
                // For simplicity: just forward to any component whose kind contains the
                // action's first segment (namespace).
                let namespace = action.split('/').next().unwrap_or("");
                e.kind.to_lowercase().contains(&namespace.to_lowercase())
            }).map(|e| e.ptr())
        };

        let result_value = match target_ptr {
            None => {
                log::warn!("[host/router] no component found for action '{action}'");
                json!({ "error": format!("no handler for action '{action}'") })
            }
            Some(ptr) => {
                // Forward the raw request to the target component
                match unsafe {
                    let res = ((*ptr).handle)(ptr, req);
                    let payload = if res.payload.is_null() || res.payload_len == 0 {
                        &[] as &[u8]
                    } else {
                        std::slice::from_raw_parts(res.payload, res.payload_len as usize)
                    };
                    let v: Value = serde_json::from_slice(payload).unwrap_or(Value::Null);
                    ((*ptr).free_response)(ptr, res);
                    v
                } {
                    v => v,
                }
            }
        };

        // Allocate response bytes for the ABI response
        let fmt         = "std/json";
        let bytes       = serde_json::to_vec(&result_value).unwrap_or_default();
        let len         = bytes.len() as u32;
        let payload_ptr = Box::into_raw(bytes.into_boxed_slice()) as *mut u8;

        AbiResponse {
            id:          req_id,
            format:      fmt.as_ptr(),
            format_len:  fmt.len() as u32,
            payload:     payload_ptr,
            payload_len: len,
        }
    })
}

// ── Main run loop ─────────────────────────────────────────────────────────────

/// Orchestrate the entire host lifecycle.
pub fn run(cfg: HostConfig) -> Result<()> {
    // 1. Create component registry and routing host
    let registry = registry::new_registry();
    let mut host = make_routing_host(registry.clone());

    // 2. Load plugins
    let mut catalog = Catalog::load(&mut host, &cfg.plugins)
        .context("loading plugins")?;
    if catalog.entries.is_empty() {
        log::warn!("[runner] no plugins loaded — running in demo mode");
    }

    // 3. Instantiate servers
    for server in &cfg.servers {
        if let Err(e) = registry::instantiate_and_register(&mut catalog, &registry, server) {
            log::error!("[runner] failed to instantiate '{}': {e}", server.name);
        }
    }

    // Log what we have
    for (name, kind) in registry::describe(&registry) {
        log::info!("[runner] registered '{name}' ({kind})");
    }

    // 4. Build and start hub
    let mut hub = MessageHub::new(cfg.hub, registry.clone())
        .context("building hub")?;
    hub.start().context("starting hub")?;
    log::info!("[runner] hub started");

    // 5. Set up Ctrl+C shutdown
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        log::info!("[runner] shutting down…");
        r.store(false, Ordering::SeqCst);
    }).context("setting Ctrl+C handler")?;

    // 6. Main loop — REST polling + hub demo events
    log::info!("[runner] entering main loop (Ctrl+C to stop)");
    let mut tick: u64 = 0;

    while running.load(Ordering::SeqCst) {
        tick += 1;

        // ── Poll RestServer ──────────────────────────────────────────────
        if let Some(rest_ptr) = registry::find_by_name(&registry, "rest-server") {
            match call_json(rest_ptr, "rest/Poll", json!({})) {
                Ok(poll_resp) => {
                    if let Some(requests) = poll_resp["requests"].as_array() {
                        for req in requests {
                            let id     = req["id"].as_u64().unwrap_or(0);
                            let action = req["action"].as_str().unwrap_or("").to_owned();
                            let body   = req["body"].clone();

                            // Route to the target component
                            let (status, resp_body) = route_action(&registry, &action, body);

                            // Send response back to RestServer
                            let _ = call_json(
                                rest_ptr,
                                "rest/Respond",
                                json!({ "id": id, "status": status, "body": resp_body }),
                            );
                        }
                    }
                }
                Err(e) => log::warn!("[runner] rest/Poll error: {e}"),
            }
        }

        // ── Hub demo: emit a log entry every 5 seconds ───────────────────
        if tick % 50 == 0 {
            let envelope = Envelope::from_json(
                next_id(),
                None,
                "log/Entry",
                json!({
                    "level":   "info",
                    "source":  "host",
                    "message": format!("heartbeat tick={tick}"),
                }),
            );
            if let Err(e) = hub.submit(envelope) {
                log::warn!("[runner] hub submit error: {e}");
            }
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    // 7. Shutdown
    hub.stop().ok();
    log::info!("[runner] stopped");
    Ok(())
}

// ── Action routing ────────────────────────────────────────────────────────────

/// Route an action to the appropriate component by action namespace.
///
/// Returns `(http_status, response_body)`.
fn route_action(
    registry: &ComponentRegistry,
    action:   &str,
    params:   Value,
) -> (u16, Value) {
    let namespace = action.split('/').next().unwrap_or("");

    let ptr_opt: Option<*mut AbiComponent> = {
        let guard = registry.lock().unwrap();
        guard.iter().find(|e| {
            e.kind.to_lowercase().contains(&namespace.to_lowercase())
        }).map(|e| e.ptr())
    };

    match ptr_opt {
        None => (404, json!({ "error": format!("no handler for action '{action}'") })),
        Some(ptr) => {
            match call_json(ptr, action, params) {
                Ok(v)  => (200, v),
                Err(e) => (500, json!({ "error": e.to_string() })),
            }
        }
    }
}
