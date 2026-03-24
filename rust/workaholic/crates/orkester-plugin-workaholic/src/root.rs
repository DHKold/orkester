use orkester_plugin::{
    abi::{AbiComponent, AbiHost},
    sdk::{AbiComponentBuilder, ComponentMetadata, Host, PluginComponent, Result},
};

use crate::{
    catalog_server::{config::CatalogServerConfig, CatalogServer},
    workflow_server::{config::WorkflowServerConfig, WorkflowServer},
};

// ── RootComponent ─────────────────────────────────────────────────────────────

/// Root component of the workaholic plugin.
///
/// Implemented manually (not via `#[component]`) so we can capture the host
/// pointer and forward it to child component factories.
pub struct RootComponent {
    host_ptr: *mut AbiHost,
}

// SAFETY: the host pointer is valid for the process lifetime and is only read
// (never written) via the `host_ptr_usize` capture below.
unsafe impl Send for RootComponent {}

impl RootComponent {
    pub fn new(host_ptr: *mut AbiHost) -> Self {
        Self { host_ptr }
    }
}

impl PluginComponent for RootComponent {
    fn get_metadata() -> ComponentMetadata {
        ComponentMetadata {
            kind:        "workaholic/Root:1.0".into(),
            name:        "WorkaholicRoot".into(),
            description: "Root component of the workaholic workflow plugin.".into(),
        }
    }

    fn to_abi(self) -> AbiComponent {
        // Capture the host pointer as usize so the closure is `Send`.
        let host_ptr_usize: usize = self.host_ptr as usize;

        AbiComponentBuilder::new()
            .with_metadata(Self::get_metadata())
            // ── CatalogServer ──────────────────────────────────────
            .with_factory(
                "workaholic/CatalogServer:1.0",
                move |_root: &mut Self, cfg: CatalogServerConfig| -> Result<CatalogServer> {
                    let ptr = host_ptr_usize as *mut AbiHost;
                    // SAFETY: ptr is valid for the process lifetime.
                    let host = unsafe { Host::from_abi(ptr) };
                    Ok(CatalogServer::new(cfg, host))
                },
                CatalogServer::get_metadata,
            )
            // ── WorkflowServer ─────────────────────────────────────
            .with_factory(
                "workaholic/WorkflowServer:1.0",
                move |_root: &mut Self, cfg: WorkflowServerConfig| -> Result<WorkflowServer> {
                    let ptr = host_ptr_usize as *mut AbiHost;
                    // SAFETY: ptr is valid for the process lifetime.
                    let host = unsafe { Host::from_abi(ptr) };
                    Ok(WorkflowServer::new(cfg, host))
                },
                WorkflowServer::get_metadata,
            )
            .build(self)
    }
}
