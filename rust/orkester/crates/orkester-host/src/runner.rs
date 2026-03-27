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
    abi::{AbiComponent, AbiRequest, AbiResponse},
    hub::MessageHub,
    sdk::Host,
};

use crate::{
    catalog::Catalog,
    config::HostConfig,
    registry::{self, ComponentRegistry},
};

// ── ABI call helpers ─────────────────────────────────────────────────────────

/// Shared list of raw plugin root component pointers (stored as `usize` for
/// `Send` compatibility).  Used to forward `orkester/CreateComponent` calls
/// to the correct plugin's factory methods.
type PluginRoots = Arc<Mutex<Vec<usize>>>;


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
/// Routing strategy: the action's first path segment (the "namespace") is
/// matched case-insensitively against the registered component **name**.
/// Names are set in the config `servers[].name` field and are fully under
/// operator control, making them a reliable routing key.
///
/// Special actions handled directly by the host before namespace routing:
/// - `orkester/GetComponent`    — look up a component pointer by name
/// - `orkester/CreateComponent` — create a new component via a plugin factory
fn make_routing_host(registry: ComponentRegistry, plugin_roots: PluginRoots) -> Host {
    Host::with_callback(move |req: AbiRequest| -> AbiResponse {
        let payload = unsafe {
            if req.payload.is_null() || req.payload_len == 0 {
                &[] as &[u8]
            } else {
                std::slice::from_raw_parts(req.payload, req.payload_len as usize)
            }
        };

        let envelope: Value = serde_json::from_slice(payload).unwrap_or(Value::Null);
        let action = envelope["action"].as_str().unwrap_or("");
        let req_id = req.id;

        // ── orkester/GetComponent ─────────────────────────────────────────
        if action == "orkester/GetComponent" {
            let cname = envelope["params"]["name"].as_str().unwrap_or("");
            let result = {
                let guard = registry.lock().unwrap();
                guard.iter().find(|e| e.name == cname).map(|e| {
                    json!({ "ptr": e.ptr() as usize, "kind": e.kind, "name": e.name })
                })
            };
            let body = result.unwrap_or_else(|| {
                log::warn!("[host/registry] orkester/GetComponent: no component named '{cname}'");
                json!({ "error": format!("component '{cname}' not found") })
            });
            let bytes = serde_json::to_vec(&body).unwrap_or_default();
            let fmt = "std/json";
            let len = bytes.len() as u32;
            let ptr = Box::into_raw(bytes.into_boxed_slice()) as *mut u8;
            return AbiResponse {
                id: req_id,
                format: fmt.as_ptr(),
                format_len: fmt.len() as u32,
                payload: ptr,
                payload_len: len,
            };
        }

        // ── orkester/CreateComponent ──────────────────────────────────────
        // Forward to each plugin root's factory until one succeeds.
        if action == "orkester/CreateComponent" {
            let kind = envelope["params"]["kind"].as_str().unwrap_or("");
            // Re-serialize the request for calling into the plugin root.
            let root_body = json!({
                "action": "orkester/CreateComponent",
                "params": envelope["params"]
            });
            let root_bytes = serde_json::to_vec(&root_body).unwrap_or_default();
            let fmt = "std/json";

            let roots = plugin_roots.lock().unwrap();
            for &root_usize in roots.iter() {
                let root_ptr = root_usize as *mut AbiComponent;
                let root_req = AbiRequest {
                    id:          req_id,
                    format:      fmt.as_ptr(),
                    format_len:  fmt.len() as u32,
                    payload:     root_bytes.as_ptr(),
                    payload_len: root_bytes.len() as u32,
                };
                let res = unsafe { ((*root_ptr).handle)(root_ptr, root_req) };
                // Check if the response is in component format.
                let res_fmt = unsafe {
                    if res.format.is_null() || res.format_len == 0 { "" }
                    else {
                        let s = std::slice::from_raw_parts(
                            res.format as *const u8, res.format_len as usize
                        );
                        std::str::from_utf8(s).unwrap_or("")
                    }
                };
                if res_fmt == "orkester/component" {
                    // Extract pointer, free root's buffer, re-allocate for caller.
                    let payload = unsafe {
                        std::slice::from_raw_parts(res.payload, res.payload_len as usize)
                    };
                    let mut addr = [0u8; std::mem::size_of::<usize>()];
                    let len = addr.len().min(payload.len());
                    addr[..len].copy_from_slice(&payload[..len]);
                    let component_ptr = usize::from_le_bytes(addr);
                    unsafe { ((*root_ptr).free_response)(root_ptr, res) };

                    log::debug!("[host/factory] created component of kind '{kind}'");
                    let new_payload = component_ptr.to_le_bytes().to_vec();
                    let new_len = new_payload.len() as u32;
                    let new_ptr = Box::into_raw(new_payload.into_boxed_slice()) as *mut u8;
                    let comp_fmt = "orkester/component";
                    return AbiResponse {
                        id:          req_id,
                        format:      comp_fmt.as_ptr(),
                        format_len:  comp_fmt.len() as u32,
                        payload:     new_ptr,
                        payload_len: new_len,
                    };
                }
                // This plugin didn't provide the factory — free the error response.
                unsafe { ((*root_ptr).free_response)(root_ptr, res) };
            }

            log::warn!("[host/factory] no plugin provides factory for kind '{kind}'");
            let body = json!({ "error": format!("no factory for component kind '{kind}'") });
            let bytes = serde_json::to_vec(&body).unwrap_or_default();
            let err_fmt = "std/json";
            let len = bytes.len() as u32;
            let ptr = Box::into_raw(bytes.into_boxed_slice()) as *mut u8;
            return AbiResponse {
                id:          req_id,
                format:      err_fmt.as_ptr(),
                format_len:  err_fmt.len() as u32,
                payload:     ptr,
                payload_len: len,
            };
        }

        // ── Namespace routing ─────────────────────────────────────────────
        let namespace = action.split('/').next().unwrap_or("");
        let target_ptr: Option<*mut AbiComponent> = {
            let guard = registry.lock().unwrap();
            guard.iter().find(|e| {
                e.name.to_lowercase().contains(&namespace.to_lowercase())
            }).map(|e| e.ptr())
        };

        let result_value = match target_ptr {
            None => {
                log::warn!(
                    "[host/router] no component found for action '{action}' \
                     (namespace='{namespace}') — check that the component is registered in servers"
                );
                json!({ "error": format!("no handler for action '{action}'") })
            }
            Some(ptr) => {
                log::debug!("[host/router] routing action '{action}' to component");
                unsafe {
                    let res = ((*ptr).handle)(ptr, req);
                    let payload = if res.payload.is_null() || res.payload_len == 0 {
                        &[] as &[u8]
                    } else {
                        std::slice::from_raw_parts(res.payload, res.payload_len as usize)
                    };
                    let v: Value = serde_json::from_slice(payload).unwrap_or(Value::Null);
                    ((*ptr).free_response)(ptr, res);
                    v
                }
            }
        };

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
    // 1. Create component registry, plugin roots, and routing host
    let registry    = registry::new_registry();
    let plugin_roots: PluginRoots = Arc::new(Mutex::new(Vec::new()));
    let mut host    = make_routing_host(registry.clone(), plugin_roots.clone());

    // 2. Load plugins
    let mut catalog = Catalog::load(&mut host, &cfg.plugins).context("loading plugins")?;
    if catalog.components.is_empty() {
        log::warn!("[runner] no plugins loaded — running in demo mode");
    }

    // 3. Instantiate servers
    for server in &cfg.servers {
        if let Err(e) = registry::instantiate_and_register(&mut catalog, &registry, server) {
            log::error!("[runner] Failed to instantiate '{}': {e}", server.name);
        }
    }

    // Log what we have
    for (name, kind) in registry::describe(&registry) {
        log::info!("[runner] Registered '{name}' ({kind})");
    }

    // 4. Build and start hub
    let mut hub = MessageHub::new(cfg.hub, registry.clone())
        .context("building hub")?;
    hub.start().context("starting hub")?;
    log::info!("[runner] Hub started");

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
            e.name.to_lowercase().contains(&namespace.to_lowercase())
        }).map(|e| e.ptr())
    };

    match ptr_opt {
        None => {
            log::warn!(
                "[runner/rest] No component for action '{action}' (namespace='{namespace}')"
            );
            (404, json!({ "error": format!("No handler for action '{action}'") }))
        }
        Some(ptr) => {
            log::debug!("[runner/rest] Routing HTTP action '{action}'");
            match call_json(ptr, action, params) {
                Ok(v)  => {
                    if v.get("error").is_some() {
                        log::warn!("[runner/rest] Action '{action}' returned error: {}", v["error"]);
                        (400, v)
                    } else {
                        (200, v)
                    }
                }
                Err(e) => {
                    log::error!("[runner/rest] Action '{action}' failed: {e}");
                    (500, json!({ "error": e.to_string() }))
                }
            }
        }
    }
}
