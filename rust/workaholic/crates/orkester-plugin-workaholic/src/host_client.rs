use orkester_plugin::sdk::{self, Host, message::Request};

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
        use orkester_plugin::abi::AbiHost;
        let req = Request { action: action.to_string(), params };
        // SAFETY: ptr was obtained from a Host::from_abi and is valid for the
        // lifetime of the plugin.
        let ptr = self.ptr as *mut AbiHost;
        let mut host = unsafe { Host::from_abi(ptr) };
        host.handle(&req)
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


