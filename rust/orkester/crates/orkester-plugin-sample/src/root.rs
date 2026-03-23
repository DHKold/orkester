use orkester_plugin::{
    abi::{AbiComponent, AbiHost},
    sdk::{AbiComponentBuilder, ComponentMetadata, PluginComponent, Result},
};

use crate::{
    log_server::{LoggingServer, LoggingServerConfig},
    ping::PingServer,
    rest_server::{RestServer, RestServerConfig},
};

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

impl RootComponent {
    /// Called by `export_plugin_root_with_host!` instead of `Default::default`.
    pub fn new(host_ptr: *mut AbiHost) -> Self {
        Self { host_ptr }
    }
}

impl PluginComponent for RootComponent {
    fn get_metadata() -> ComponentMetadata {
        ComponentMetadata {
            kind:        "sample/Root:1.0".into(),
            name:        "SampleRoot".into(),
            description: "Root component of the Orkester sample plugin.".into(),
        }
    }

    fn to_abi(self) -> AbiComponent {
        // Convert to usize before the closure so no raw pointer is captured.
        // A usize is always Send + Sync, and we restore the pointer inside.
        let host_ptr_usize: usize = self.host_ptr as usize;

        AbiComponentBuilder::new()
            .with_metadata(Self::get_metadata())
            // ── PingServer ───────────────────────────────────────────────
            .with_factory(
                "sample/PingServer:1.0",
                |_root: &mut Self, _cfg: serde_json::Value| -> Result<PingServer> {
                    Ok(PingServer::default())
                },
                PingServer::get_metadata,
            )
            // ── LoggingServer ────────────────────────────────────────────
            .with_factory(
                "sample/LoggingServer:1.0",
                |_root: &mut Self, cfg: LoggingServerConfig| LoggingServer::new(cfg),
                LoggingServer::get_metadata,
            )
            // ── RestServer ───────────────────────────────────────────────
            .with_factory(
                "sample/RestServer:1.0",
                move |_root: &mut Self, cfg: RestServerConfig| -> Result<RestServer> {
                    // SAFETY: host_ptr_usize was cast from a valid *mut AbiHost that
                    // remains live for the process lifetime.
                    let ptr = host_ptr_usize as *mut AbiHost;
                    Ok(RestServer::new(cfg, ptr))
                },
                RestServer::get_metadata,
            )
            .build(self)
    }
}

