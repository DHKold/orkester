use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::Value;
use orkester_plugin::hub::ComponentEntry;

/// Kind prefix that identifies a LoggingServer component.
pub const LOGGING_SERVER_KIND_PREFIX: &str = "logging/LoggingServer";

const STARTUP_BUFFER_CAPACITY: usize = 1024;

/// Host-side bridge between the SDK logging path and the active LoggingServer.
///
/// Lifecycle:
/// 1. Created before any plugins are loaded (`HostLogBridge::new()`).
/// 2. Passed (via `Arc`) into the routing closure of `make_routing_host`.
/// 3. After the LoggingServer component is instantiated, `connect(entry)`
///    flushes the startup buffer and switches to live delivery.
pub struct HostLogBridge {
    startup_buffer: Mutex<Vec<Value>>,
    connected:      AtomicBool,
    server:         Mutex<Option<ComponentEntry>>,
}

impl HostLogBridge {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            startup_buffer: Mutex::new(Vec::with_capacity(STARTUP_BUFFER_CAPACITY)),
            connected:      AtomicBool::new(false),
            server:         Mutex::new(None),
        })
    }

    /// Called from the routing closure for every `log/*` format request.
    pub fn submit(&self, payload: &[u8]) {
        if self.connected.load(Ordering::Acquire) {
            self.deliver_direct(payload);
        } else {
            self.buffer_append(payload);
        }
    }

    /// Wire the bridge to the live LoggingServer.  Flushes the startup buffer.
    pub fn connect(&self, entry: ComponentEntry) {
        // Hold both locks to prevent records from slipping between buffer and
        // delivery while we swap to connected mode.
        let mut buf = self.startup_buffer.lock().unwrap_or_else(|e| e.into_inner());
        let mut srv = self.server.lock().unwrap_or_else(|e| e.into_inner());
        *srv = Some(entry);
        self.connected.store(true, Ordering::Release);
        let srv_ref = srv.as_ref().unwrap();
        for record in buf.drain(..) {
            let _ = srv_ref.call_json("logging/Ingest", record);
        }
    }

    fn buffer_append(&self, payload: &[u8]) {
        let Ok(record) = serde_json::from_slice::<Value>(payload) else { return };
        let mut buf = self.startup_buffer.lock().unwrap_or_else(|e| e.into_inner());
        if buf.len() < STARTUP_BUFFER_CAPACITY {
            buf.push(record);
        }
    }

    fn deliver_direct(&self, payload: &[u8]) {
        let Ok(record) = serde_json::from_slice::<Value>(payload) else { return };
        let srv = self.server.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = srv.as_ref() {
            let _ = s.call_json("logging/Ingest", record);
        }
    }
}
