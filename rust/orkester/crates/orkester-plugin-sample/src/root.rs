use orkester_plugin::{abi::AbiHost, prelude::*};

use crate::rest_server::{RestServer, RestServerConfig};

// ── RootComponent ─────────────────────────────────────────────────────────────

/// Root component of the sample plugin.
///
/// Implemented manually (instead of with `#[component]`) so we can capture the
/// host pointer at construction time and pass it to `RestServer`.
pub struct RootComponent {
    host_ptr: *mut AbiHost,
}

// SAFETY: host_ptr is valid for the process lifetime; it is only used to build
// child components synchronously and is never written after construction.
unsafe impl Send for RootComponent {}

#[component(
    kind = "sample/Root:1.0",
    name = "Sample Root Component",
    description = "Root component for the Sample plugin, providing a REST server.",
)]
impl RootComponent {
    /// Called by `export_plugin_root_with_host!` instead of `Default::default`.
    pub fn new(host_ptr: *mut AbiHost) -> Self {
        Self { host_ptr }
    }

    /// Factory method for the REST server component, which receives the host pointer so it can call back into the host router.
    #[factory("sample/RestServer:1.0")]
    fn create_rest_server(&mut self, config: RestServerConfig) -> Result<RestServer> {
        Ok(RestServer::new(config, self.host_ptr))
    }
}

