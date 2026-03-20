use orkester_plugin::{
    abi::AbiComponent,
    sdk::message::{Serializer, Deserializer},
    sdk::Result,
};
use serde_json::Value;

// ── Live server ───────────────────────────────────────────────────────────────

/// A running server — a live component that accepts requests.
pub struct Server {
    pub name: String,
    pub kind: String,
    component: *mut AbiComponent,
}

// SAFETY: single-threaded host; we don't share across threads.
unsafe impl Send for Server {}

impl Server {
    pub fn new(name: String, kind: String, component: *mut AbiComponent) -> Self {
        Self { name, kind, component }
    }

    /// Send a request to this server and return the raw JSON response.
    pub fn handle(&mut self, action: &str, params: Value) -> Result<Value> {
        let envelope = serde_json::json!({ "action": action, "params": params });
        let req = Serializer::json(&envelope);
        let raw_res = unsafe { ((*self.component).handle)(self.component, req.as_abi()) };
        Deserializer::value(self.component, raw_res)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        unsafe { ((*self.component).free)(self.component) };
    }
}

// ── Hub ───────────────────────────────────────────────────────────────────────

/// Simple synchronous message hub.
///
/// Routes messages from one component to the correct server(s) based on
/// the action prefix.  Components send messages to the host via the
/// [`Host::handle`] back-channel; the hub receives them and forwards them.
pub struct Hub {
    servers: Vec<Server>,
}

impl Hub {
    pub fn new(servers: Vec<Server>) -> Self {
        Self { servers }
    }

    /// Route an action + params to every server whose kind prefix matches.
    ///
    /// Returns the first successful response, or an error if none matched.
    pub fn route(&mut self, action: &str, params: Value) -> Result<Value> {
        for server in &mut self.servers {
            // Simple prefix routing: "sample/Log" goes to anything whose kind
            // starts with "sample/Logger".
            if action_matches(action, &server.kind) {
                return server.handle(action, params.clone());
            }
        }
        Err(format!("no server handles action '{action}'").into())
    }

    pub fn servers(&self) -> &[Server] {
        &self.servers
    }

    pub fn shutdown(self) {
        // Dropping self drops all servers which call component.free().
        eprintln!("[hub] shutdown — releasing {} server(s)", self.servers.len());
    }
}

fn action_matches(action: &str, kind: &str) -> bool {
    // Extract the namespace prefix from the kind (e.g. "sample/Logger:1.0" → "sample").
    let kind_prefix = kind.split('/').next().unwrap_or(kind);
    let action_prefix = action.split('/').next().unwrap_or(action);
    kind_prefix.eq_ignore_ascii_case(action_prefix)
}
