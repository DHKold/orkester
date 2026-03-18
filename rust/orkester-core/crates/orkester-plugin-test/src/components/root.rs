//! Root component -- the plugin's entry point and child factory.
//!
//! Responsibilities:
//! 1. `Ping`             -- returns `"pong"` (liveness probe).
//! 2. `Metadata`         -- returns JSON describing the plugin and component kinds.
//! 3. `CreateComponent`  -- allocates a child component and returns its pointer.
//!
//! Every request is also logged to the host as a demonstration of the callback path.

use std::sync::atomic::{AtomicU32, Ordering};

use serde::{Deserialize, Serialize};
use orkester_plugin::{
    abi,
    sdk::{require_json, alloc_component, ComponentHandler, HandlerResponse, Request,
          COMPONENT_KIND_PLUGIN, ERROR_INTERNAL, ERROR_INVALID_REQUEST,
          FLAG_UTF8, MSG_TYPE_JSON, PROTOCOL_V1},
};

use orkester_plugin::sdk::ComponentKind;
use crate::constants::{KIND_CALCULATOR, KIND_COUNTER, KIND_ECHO, KIND_GREETER};
use super::{calculator::Calculator, counter::Counter, echo::Echo, greeter::Greeter};

// Plugin-wide monotonic ID allocator; root is always 0.
static NEXT_ID: AtomicU32 = AtomicU32::new(1);
fn next_id() -> u32 { NEXT_ID.fetch_add(1, Ordering::Relaxed) }

// ---- Wire types ------------------------------------------------------------

#[derive(Deserialize)]
struct RootReq {
    #[serde(rename = "type")]
    request_type: String,
    #[serde(default)]
    kind: Option<String>,
}

#[derive(Serialize)]
struct Metadata {
    name: &'static str,
    version: &'static str,
    protocol: u32,
    component_kinds: Vec<ComponentKind>,
}

// ---- Root handler ----------------------------------------------------------

pub struct Root {
    host: *mut abi::Host,
}

// SAFETY: host pointer is valid for the plugin's lifetime.
unsafe impl Send for Root {}
unsafe impl Sync for Root {}

impl Root {
    pub fn new(host: *mut abi::Host) -> Self { Self { host } }
}

impl ComponentHandler for Root {
    fn handle(&mut self, req: Request) -> HandlerResponse {
        let parsed: RootReq = match require_json(&req) {
            Ok(v)  => v,
            Err(e) => return e,
        };

        log_to_host(self.host, req.id(), &format!("root received: {}", parsed.request_type));

        match parsed.request_type.as_str() {
            "Ping" => HandlerResponse::string("pong"),

            "Metadata" => HandlerResponse::json(&Metadata {
                name: "orkester-plugin-test",
                version: env!("CARGO_PKG_VERSION"),
                protocol: PROTOCOL_V1,
                component_kinds: vec![
                    ComponentKind::new(COMPONENT_KIND_PLUGIN, "Root",       "Plugin entry point"),
                    ComponentKind::new(KIND_ECHO,             "Echo",       "Reflects any payload back unchanged"),
                    ComponentKind::new(KIND_COUNTER,          "Counter",    "Stateful i64 counter"),
                    ComponentKind::new(KIND_GREETER,          "Greeter",    "Multi-language greeter with host callbacks"),
                    ComponentKind::new(KIND_CALCULATOR,       "Calculator", "Binary arithmetic"),
                ],
            }),

            "CreateComponent" => {
                let kind = match parsed.kind.as_deref() {
                    Some(k) => k,
                    None    => return HandlerResponse::error(ERROR_INVALID_REQUEST, "CreateComponent requires 'kind'"),
                };
                let id = next_id();
                let ptr: *mut abi::Component = match kind {
                    "Echo"       => alloc_component(id, KIND_ECHO,       0, Echo),
                    "Counter"    => alloc_component(id, KIND_COUNTER,    0, Counter::new()),
                    "Greeter"    => alloc_component(id, KIND_GREETER,    0, Greeter::new(self.host)),
                    "Calculator" => alloc_component(id, KIND_CALCULATOR, 0, Calculator),
                    other => return HandlerResponse::error(
                        ERROR_INVALID_REQUEST,
                        format!("unknown kind {other:?}; expected Echo, Counter, Greeter, Calculator"),
                    ),
                };
                if ptr.is_null() {
                    return HandlerResponse::error(ERROR_INTERNAL, "allocation failed");
                }
                log_to_host(self.host, req.id(), &format!("created {kind} (id={id})"));
                HandlerResponse::pointer(ptr)
            }

            other => HandlerResponse::error(
                ERROR_INVALID_REQUEST,
                format!("unknown request type {other:?}; expected Ping, Metadata, CreateComponent"),
            ),
        }
    }
}

// ---- Host log callback -----------------------------------------------------

fn log_to_host(host: *mut abi::Host, id: u64, message: &str) {
    if host.is_null() { return; }
    #[derive(Serialize)]
    struct Log<'a> { #[serde(rename = "type")] t: &'static str, message: &'a str }
    let Ok(payload) = serde_json::to_vec(&Log { t: "Log", message }) else { return };
    let req = abi::Request {
        id, format: MSG_TYPE_JSON, flags: FLAG_UTF8,
        payload: payload.as_ptr(), len: payload.len() as u32,
    };
    unsafe {
        let res = ((*host).handle)(host, req);
        ((*host).free_response)(host, res);
    }
}
