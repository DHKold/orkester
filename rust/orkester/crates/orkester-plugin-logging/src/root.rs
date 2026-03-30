use orkester_plugin::{abi::AbiHost, prelude::*};

use crate::server::{LoggingServer, LoggingServerConfig};

pub struct LoggingRoot {
    host_ptr: *mut AbiHost,
}

// SAFETY: The host ABI pointer is valid for the process lifetime and callable
// from any thread (same guarantee as `HostRef: Send + Sync`).
unsafe impl Send for LoggingRoot {}

#[component(
    kind = "logging/LoggingRoot:1.0",
    name = "Logging Root",
    description = "Root component for the Logging plugin."
)]
impl LoggingRoot {
    pub fn new(host_ptr: *mut AbiHost) -> Self {
        Self { host_ptr }
    }

    #[factory("logging/LoggingServer:1.0")]
    fn create_logging_server(&mut self, config: LoggingServerConfig) -> Result<LoggingServer> {
        Ok(LoggingServer::new(config, self.host_ptr))
    }
}
