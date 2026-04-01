use orkester_plugin::abi::AbiHost;
use orkester_plugin::prelude::*;

use crate::rest_server::{RestServer, RestServerConfig};

pub struct RootComponent {
    host_ptr: *mut AbiHost,
}

// SAFETY: The ABI host pointer is valid for the process lifetime and callable
// from any thread (same guarantee as HostRef: Send + Sync).
unsafe impl Send for RootComponent {}

#[component(
    kind        = "core/Root:1.0",
    name        = "Orkester Core Root",
    description = "Root component providing the Axum-based REST server factory.",
)]
impl RootComponent {
    pub fn new(host_ptr: *mut AbiHost) -> Self {
        Self { host_ptr }
    }

    #[factory("core/RestServer:1.0")]
    fn create_rest_server(&mut self, config: RestServerConfig) -> Result<RestServer> {
        Ok(RestServer::new(config, self.host_ptr))
    }
}
