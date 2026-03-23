use std::sync::{Arc, Mutex};

use serde::Deserialize;
use serde_json::Value;

use super::Dispatcher;
use crate::{
    abi::{AbiComponent, AbiRequest},
    hub::{envelope::Envelope, error::DispatchError},
};

// ── ComponentEntry — the host-side registry entry ─────────────────────────────

/// A live ABI component registered by the host.
///
/// The host creates one `ComponentEntry` per instantiated server after plugin
/// loading, and inserts it into the [`ComponentRegistry`].  The
/// `ComponentsDispatcher` then delivers envelopes to matching entries.
pub struct ComponentEntry {
    pub name: String,
    pub kind: String,
    /// Raw pointer to the ABI vtable.  Valid for the process lifetime of the
    /// owning [`crate::sdk::LoadedPlugin`].
    ptr: *mut AbiComponent,
}

// SAFETY: ComponentEntry is only accessed from within Mutex-guarded code or
// from the single host-management thread.  No two threads call handle() on
// the same AbiComponent simultaneously.
unsafe impl Send for ComponentEntry {}
unsafe impl Sync for ComponentEntry {}

impl ComponentEntry {
    pub fn new(name: impl Into<String>, kind: impl Into<String>, ptr: *mut AbiComponent) -> Self {
        Self { name: name.into(), kind: kind.into(), ptr }
    }

    /// Raw ABI pointer.  The caller is responsible for calling it safely.
    pub fn ptr(&self) -> *mut AbiComponent { self.ptr }

    /// Translate a hub [`Envelope`] into an ABI request and deliver it to this
    /// component (fire-and-forget).
    ///
    /// The envelope `kind` becomes the `action` field; `payload` must be valid
    /// JSON (will be embedded as-is under `params`).  The component's response
    /// is freed immediately — callers that need a return value must use
    /// `call_json` instead.
    pub fn deliver(&self, envelope: &Envelope) -> Result<(), String> {
        // Build the standard JSON envelope: { "action": "...", "params": ... }
        let params: Value = serde_json::from_slice(&*envelope.payload)
            .unwrap_or(Value::Null);
        // Normalize null → {} so handlers with all-default fields can deserialize.
        let params = if params.is_null() { serde_json::json!({}) } else { params };
        let body = serde_json::json!({ "action": &*envelope.kind, "params": params });
        let body_bytes = serde_json::to_vec(&body).map_err(|e| e.to_string())?;

        let fmt = "std/json";
        let req = AbiRequest {
            id:          envelope.id,
            format:      fmt.as_ptr(),
            format_len:  fmt.len() as u32,
            payload:     body_bytes.as_ptr(),
            payload_len: body_bytes.len() as u32,
        };

        unsafe {
            let res = ((*self.ptr).handle)(self.ptr, req);
            ((*self.ptr).free_response)(self.ptr, res);
        }
        Ok(())
    }

    /// Call this component with a JSON payload and return the JSON response.
    /// Use for synchronous request-response calls (e.g. from the host loop).
    pub fn call_json(&self, action: &str, params: Value) -> Result<Value, String> {
        let body = serde_json::json!({ "action": action, "params": params });
        let body_bytes = serde_json::to_vec(&body).map_err(|e| e.to_string())?;

        let fmt = "std/json";
        let req = AbiRequest {
            id:          0,
            format:      fmt.as_ptr(),
            format_len:  fmt.len() as u32,
            payload:     body_bytes.as_ptr(),
            payload_len: body_bytes.len() as u32,
        };

        let value = unsafe {
            let res = ((*self.ptr).handle)(self.ptr, req);
            let payload = if res.payload.is_null() || res.payload_len == 0 {
                &[] as &[u8]
            } else {
                std::slice::from_raw_parts(res.payload, res.payload_len as usize)
            };
            let v: Value = serde_json::from_slice(payload).unwrap_or(Value::Null);
            ((*self.ptr).free_response)(self.ptr, res);
            v
        };
        Ok(value)
    }
}

// ── ComponentRegistry ─────────────────────────────────────────────────────────

/// Shared, mutable registry of live components.
///
/// Created by the host before building the hub; passed to [`ComponentsDispatcher`]
/// at hub construction time.  The host populates it after instantiating servers.
pub type ComponentRegistry = Arc<Mutex<Vec<ComponentEntry>>>;

// ── Target selector ───────────────────────────────────────────────────────────

/// Selects an entry from the registry by name and/or kind-prefix.
#[derive(Debug, Clone, Deserialize)]
pub struct ComponentTarget {
    /// Exact display name match (the server name declared in host config).
    pub name: Option<String>,
    /// Kind prefix match (e.g. `"sample/Logger"` matches `"sample/Logger:1.0"`).
    pub kind: Option<String>,
}

impl ComponentTarget {
    fn matches(&self, entry: &ComponentEntry) -> bool {
        if let Some(n) = &self.name {
            if entry.name == *n { return true; }
        }
        if let Some(k) = &self.kind {
            if entry.kind.starts_with(k.as_str()) { return true; }
        }
        false
    }
}

// ── Dispatcher ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ComponentsConfig {
    targets: Vec<ComponentTarget>,
}

/// Delivers envelopes to all registry entries matching at least one [`ComponentTarget`].
pub struct ComponentsDispatcher {
    targets:  Vec<ComponentTarget>,
    registry: ComponentRegistry,
}

impl ComponentsDispatcher {
    pub fn new(targets: Vec<ComponentTarget>, registry: ComponentRegistry) -> Self {
        Self { targets, registry }
    }

    pub fn from_config(config: &Value, registry: ComponentRegistry) -> Result<Self, String> {
        let cfg: ComponentsConfig = serde_json::from_value(config.clone())
            .map_err(|e| e.to_string())?;
        Ok(Self::new(cfg.targets, registry))
    }
}

impl Dispatcher for ComponentsDispatcher {
    fn name(&self) -> &str { "components" }

    fn dispatch(&self, envelope: Envelope) -> Result<(), DispatchError> {
        let registry = self.registry.lock().map_err(|_| DispatchError {
            dispatcher: self.name().to_owned(),
            cause: "registry mutex poisoned".to_owned(),
        })?;

        let mut delivered = 0usize;
        for entry in registry.iter() {
            if self.targets.iter().any(|t| t.matches(entry)) {
                if let Err(e) = entry.deliver(&envelope) {
                    log::warn!(
                        "[hub/components] delivery to '{}' failed: {e}",
                        entry.name
                    );
                }
                delivered += 1;
            }
        }

        log::debug!(
            "[hub/components] id={} kind='{}' → {delivered} component(s)",
            envelope.id, envelope.kind
        );
        Ok(())
    }
}
