use std::collections::HashMap;
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

    /// Send a request to this server and return the JSON response.
    pub fn handle(&mut self, action: &str, params: Value) -> Result<Value> {
        let envelope = serde_json::json!({ "action": action, "params": params });
        let req = Serializer::json(&envelope);
        let raw_res = unsafe { ((*self.component).handle)(self.component, req.as_abi()) };
        Deserializer::value(self.component, raw_res)
    }

    /// Query the component for all action names it handles.
    fn list_actions(&mut self) -> Vec<String> {
        let envelope = serde_json::json!({ "action": "orkester/ListActions", "params": null });
        let req = Serializer::json(&envelope);
        let raw_res = unsafe { ((*self.component).handle)(self.component, req.as_abi()) };
        Deserializer::json::<Vec<String>>(self.component, raw_res).unwrap_or_default()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        unsafe { ((*self.component).free)(self.component) };
    }
}

// ── Hub ───────────────────────────────────────────────────────────────────────

/// Synchronous message hub.
///
/// At construction it queries every server for its supported actions
/// (`orkester/ListActions`) and builds an exact action → server routing table.
/// `route()` dispatches each request to the correct server in O(1).
pub struct Hub {
    servers: Vec<Server>,
    /// Maps action name → indices in `self.servers`.
    routes: HashMap<String, Vec<usize>>,
}

impl Hub {
    /// Build the hub and populate the routing table from each server's action list.
    pub fn new(mut servers: Vec<Server>) -> Self {
        let mut routes = HashMap::new();
        for (i, server) in servers.iter_mut().enumerate() {
            let actions = server.list_actions();
            eprintln!(
                "[hub] '{}' ({}) — {} action(s)",
                server.name, server.kind, actions.len()
            );
            for action in actions {
                // First registered server wins for each action.
                routes.entry(action).or_insert_with(Vec::new).push(i);
            }
        }
        Self { servers, routes }
    }

    /// Dispatch `action` to the server registered for it.
    pub fn route(&mut self, action: &str, params: Value) -> Result<Value> {
        let indices = self.routes.get(action).ok_or_else(|| format!("no server handles action '{action}'"))?;
        for idx in indices {
            let server = &mut self.servers[*idx];
            match server.handle(action, params.clone()) {
                Ok(_res) => (),
                Err(err) => eprintln!("[hub] error from server '{}': {err} — trying next", server.name),
            }
        }
        Ok(serde_json::json!({ "status": "ok" }))
    }

    pub fn servers(&self) -> &[Server] { &self.servers }

    pub fn route_count(&self) -> usize { self.routes.len() }

    pub fn shutdown(self) {
        eprintln!("[hub] shutdown — releasing {} server(s)", self.servers.len());
        // Dropping self drops all Servers, which each call component.free().
    }
}
