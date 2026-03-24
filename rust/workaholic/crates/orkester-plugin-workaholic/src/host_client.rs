use orkester_plugin::{abi::{AbiComponent, AbiHost, AbiRequest}, sdk::{self, Host, message::Request}};

/// Thread-safe wrapper for calling back into the orkester host via the SDK.
///
/// Internally stores the host pointer as a `usize` to satisfy `Send + Sync`.
/// Each call creates a temporary `Host::from_abi` wrapper (zero-alloc since
/// the pointer is not owned) and delegates through `host.handle::<T, R>()`.
#[derive(Clone)]
pub struct HostClient {
    /// Raw host pointer stored as usize for Send + Sync.
    ptr: usize,
}

// SAFETY: the host is valid for the process lifetime and its callback is
// Fn + Send + Sync.
unsafe impl Send for HostClient {}
unsafe impl Sync for HostClient {}

// ── OwnedComponent ────────────────────────────────────────────────────────────

/// RAII owner of a transient `*mut AbiComponent` created on-demand via
/// `HostClient::create_component`.
///
/// The component is freed (via its `free` vtable method) when this value is
/// dropped.  Workers create one of these per task execution and drop it when
/// the task finishes.
pub struct OwnedComponent {
    ptr: *mut AbiComponent,
}

// SAFETY: the component pointer is valid for the lifetime of OwnedComponent
// and is only accessed from the holding thread.
unsafe impl Send for OwnedComponent {}

impl OwnedComponent {
    /// Call an action on this transient component and receive a typed response.
    ///
    /// # Example
    /// ```ignore
    /// let resp: RunnerExecuteResponse = runner.call("Execute", req)?;
    /// ```
    pub fn call<P, R>(&self, action: &str, params: P) -> sdk::Result<R>
    where
        P: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let body = serde_json::json!({ "action": action, "params": params });
        let bytes = serde_json::to_vec(&body).map_err(|e| -> sdk::Error { e.to_string().into() })?;
        let fmt = "std/json";
        let req = AbiRequest {
            id:          0,
            format:      fmt.as_ptr(),
            format_len:  fmt.len() as u32,
            payload:     bytes.as_ptr(),
            payload_len: bytes.len() as u32,
        };
        let res = unsafe { ((*self.ptr).handle)(self.ptr, req) };
        let payload = unsafe {
            if res.payload.is_null() || res.payload_len == 0 {
                &[] as &[u8]
            } else {
                std::slice::from_raw_parts(res.payload, res.payload_len as usize)
            }
        };
        let value: R = serde_json::from_slice(payload)
            .map_err(|e| -> sdk::Error { e.to_string().into() })?;
        unsafe { ((*self.ptr).free_response)(self.ptr, res) };
        Ok(value)
    }
}

impl Drop for OwnedComponent {
    fn drop(&mut self) {
        unsafe { ((*self.ptr).free)(self.ptr) };
    }
}

// ── ComponentRef ──────────────────────────────────────────────────────────────

/// A raw ABI component pointer obtained from the host registry.
///
/// Stored as usize so `ComponentRef` is `Send + Sync`.  The caller must ensure
/// the pointed-to component outlives all uses of this reference.
#[derive(Debug, Clone)]
pub struct ComponentRef {
    pub ptr:  usize,
    pub kind: String,
    pub name: String,
}

impl ComponentRef {
    /// Cast back to a raw `*mut AbiComponent` for ABI calls.
    ///
    /// # Safety
    /// The pointer must still be valid (i.e. the component must be alive).
    pub unsafe fn as_ptr(&self) -> *mut AbiComponent {
        self.ptr as *mut AbiComponent
    }
}

impl HostClient {
    /// Wrap the SDK `Host` instance.  The underlying pointer is extracted and
    /// the `Host` wrapper is released immediately (it does not own the ABI
    /// allocation when created via `from_abi`).
    pub fn new(host: Host) -> Self {
        Self { ptr: host.raw_ptr() as usize }
    }

    /// Dispatch an action through the host, deserializing the response as `R`.
    pub fn call<P, R>(&self, action: &str, params: P) -> sdk::Result<R>
    where
        P: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let req = Request { action: action.to_string(), params };
        // SAFETY: ptr was obtained from a Host::from_abi and is valid for the
        // lifetime of the plugin.
        let ptr = self.ptr as *mut AbiHost;
        let mut host = unsafe { Host::from_abi(ptr) };
        host.handle(&req)
    }

    /// Create a new component instance of `kind` on demand.
    ///
    /// The host routes `orkester/CreateComponent` to the plugin root's factory
    /// method, which constructs a fresh component and returns its ABI pointer.
    /// The returned [`OwnedComponent`] frees the component when dropped.
    ///
    /// Returns `None` if no factory for `kind` is registered or if creation
    /// fails.
    pub fn create_component<C: serde::Serialize>(
        &self,
        kind:   &str,
        config: C,
    ) -> Option<OwnedComponent> {
        let body = serde_json::json!({
            "action": "orkester/CreateComponent",
            "params": { "kind": kind, "config": config }
        });
        let bytes = serde_json::to_vec(&body).ok()?;
        let fmt = "std/json";
        let req = AbiRequest {
            id:          0,
            format:      fmt.as_ptr(),
            format_len:  fmt.len() as u32,
            payload:     bytes.as_ptr(),
            payload_len: bytes.len() as u32,
        };

        let host_ptr = self.ptr as *mut AbiHost;
        let res = unsafe { ((*host_ptr).handle)(host_ptr, req) };

        // The response format is "orkester/component" when a component was created.
        let fmt_str = unsafe {
            if res.format.is_null() || res.format_len == 0 {
                ""
            } else {
                let s = std::slice::from_raw_parts(res.format as *const u8, res.format_len as usize);
                std::str::from_utf8(s).unwrap_or("")
            }
        };

        if fmt_str == "orkester/component" {
            let payload = unsafe {
                if res.payload.is_null() {
                    ((*host_ptr).free_response)(host_ptr, res);
                    return None;
                }
                std::slice::from_raw_parts(res.payload, res.payload_len as usize)
            };
            let sz = std::mem::size_of::<usize>();
            if payload.len() < sz {
                unsafe { ((*host_ptr).free_response)(host_ptr, res) };
                return None;
            }
            let mut addr = [0u8; std::mem::size_of::<usize>()];
            addr.copy_from_slice(&payload[..sz]);
            let component_ptr = usize::from_le_bytes(addr) as *mut AbiComponent;
            unsafe { ((*host_ptr).free_response)(host_ptr, res) };
            if component_ptr.is_null() {
                None
            } else {
                Some(OwnedComponent { ptr: component_ptr })
            }
        } else {
            // Extract error message for logging
            let msg = unsafe {
                if res.payload.is_null() || res.payload_len == 0 {
                    "no response".to_string()
                } else {
                    let s = std::slice::from_raw_parts(res.payload, res.payload_len as usize);
                    serde_json::from_slice::<serde_json::Value>(s)
                        .ok()
                        .and_then(|v| v["error"].as_str().map(|s| s.to_string()))
                        .unwrap_or_else(|| format!("format='{fmt_str}'"))
                }
            };
            self.log("warn", "host_client",
                &format!("create_component '{}' failed: {}", kind, msg));
            unsafe { ((*host_ptr).free_response)(host_ptr, res) };
            None
        }
    }

    /// Look up a registered component by name, returning its ABI pointer and
    /// metadata.  Returns `None` if no component with that name is registered.
    pub fn get_component(&self, name: &str) -> Option<ComponentRef> {
        #[derive(serde::Deserialize)]
        struct GetComponentResponse {
            ptr:  usize,
            kind: String,
            name: String,
        }
        match self.call::<_, GetComponentResponse>(
            "orkester/GetComponent",
            serde_json::json!({ "name": name }),
        ) {
            Ok(r) => Some(ComponentRef { ptr: r.ptr, kind: r.kind, name: r.name }),
            Err(e) => {
                self.log("warn", "host_client",
                    &format!("orkester/GetComponent '{name}' failed: {e}"));
                None
            }
        }
    }

    /// Fire-and-forget log emission through the `log/Entry` hub action.
    pub fn log(&self, level: &str, source: &str, message: &str) {
        let _ = self.call::<_, serde_json::Value>(
            "log/Entry",
            serde_json::json!({
                "level":   level,
                "source":  source,
                "message": message,
            }),
        );
    }
}

