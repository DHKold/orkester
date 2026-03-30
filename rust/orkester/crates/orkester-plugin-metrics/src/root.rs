use orkester_plugin::{
    abi::AbiComponent,
    sdk::{AbiComponentBuilder, ComponentMetadata, PluginComponent, Result},
};

use crate::metrics::{MetricsServer};

/// Root component of the metrics plugin — exposes MetricsServer as a child component.
#[derive(Default)]
pub struct MetricsRoot;

impl PluginComponent for MetricsRoot {
    fn get_metadata() -> ComponentMetadata {
        ComponentMetadata {
            kind:        "metrics/Root:1.0".into(),
            name:        "Metrics Root Component".into(),
            description: "Root component for the Orkester metrics plugin.".into(),
        }
    }

    fn to_abi(self) -> AbiComponent {
        AbiComponentBuilder::new()
            .with_metadata(Self::get_metadata())
            .with_factory(
                "metrics/MetricsServer:1.0",
                |_: &mut Self, raw: serde_json::Value| -> Result<MetricsServer> {
                    let cfg = serde_json::from_value(raw).unwrap_or_default();
                    Ok(MetricsServer::new(cfg))
                },
                MetricsServer::get_metadata,
            )
            .build(self)
    }
}
