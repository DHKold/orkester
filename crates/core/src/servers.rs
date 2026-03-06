use std::sync::Arc;
use serde_json::Value;
use thiserror::Error;
use orkester_common::plugin::{PluginComponent};
use orkester_common::servers::rest::{ApiContributor, RestServerDeps};
use orkester_common::servers::state::StateServer;
use orkester_common::servers::workflow::WorkflowServer;
use orkester_common::servers::metrics::MetricsServer;
use orkester_common::servers::rest::RestServer;
use crate::plugins::LoadedPlugin;

// ── Error ──────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ServerBuildError {
    #[error("No factory named '{name}' found for {role} server")]
    FactoryNotFound { role: &'static str, name: String },
    #[error("Failed to build {role} server '{name}': {reason}")]
    BuildFailed {
        role: &'static str,
        name: String,
        reason: String,
    },
}

// ── Result ─────────────────────────────────────────────────────────────────

pub struct BuiltServers {
    pub state: Box<dyn StateServer>,
    pub workflow: Box<dyn WorkflowServer>,
    pub metrics: Box<dyn MetricsServer>,
    pub rest: Box<dyn RestServer>,
}

// ── Builder ────────────────────────────────────────────────────────────────

pub fn build_servers(
    config: &Value,
    loaded: &mut Vec<LoadedPlugin>,
) -> Result<BuiltServers, ServerBuildError> {
    // Drain all components out of every loaded plugin.
    let mut components: Vec<PluginComponent> = loaded
        .iter_mut()
        .flat_map(|lp| lp.plugin.components.drain(..))
        .collect();

    // Collect API contributors first (passed as deps to the REST factory).
    let contributors: Vec<Arc<dyn ApiContributor>> = components
        .iter_mut()
        .filter_map(|c| {
            if let PluginComponent::ApiContributor(_) = c {
                // We need to move out — swap with a placeholder isn't possible on
                // an enum with no cheap default, so we drain with retain below.
                None
            } else {
                None
            }
        })
        .collect();

    // Proper drain of ApiContributor variants.
    let mut api_contributors: Vec<Arc<dyn ApiContributor>> = Vec::new();
    let mut remaining: Vec<PluginComponent> = Vec::new();
    for c in components {
        match c {
            PluginComponent::ApiContributor(ac) => api_contributors.push(Arc::from(ac)),
            other => remaining.push(other),
        }
    }
    let _ = contributors; // replaced by api_contributors above
    let components = remaining;

    // ── State server ──────────────────────────────────────────────────────
    let state_name = server_plugin(config, "state", "basic-state-server");
    let state_cfg  = server_config(config, "state");
    let state = {
        let factory = components.iter().find_map(|c| {
            if let PluginComponent::StateServer(f) = c {
                if f.name() == state_name { Some(f) } else { None }
            } else {
                None
            }
        }).ok_or_else(|| ServerBuildError::FactoryNotFound {
            role: "state",
            name: state_name.to_string(),
        })?;
        factory.build(state_cfg).map_err(|e| ServerBuildError::BuildFailed {
            role: "state",
            name: state_name.to_string(),
            reason: e.to_string(),
        })?
    };

    // ── Workflow server ───────────────────────────────────────────────────
    let workflow_name = server_plugin(config, "workflow", "basic-workflow-server");
    let workflow_cfg  = server_config(config, "workflow");
    let workflow = {
        let factory = components.iter().find_map(|c| {
            if let PluginComponent::WorkflowServer(f) = c {
                if f.name() == workflow_name { Some(f) } else { None }
            } else {
                None
            }
        }).ok_or_else(|| ServerBuildError::FactoryNotFound {
            role: "workflow",
            name: workflow_name.to_string(),
        })?;
        factory.build(workflow_cfg).map_err(|e| ServerBuildError::BuildFailed {
            role: "workflow",
            name: workflow_name.to_string(),
            reason: e.to_string(),
        })?
    };

    // ── Metrics server ────────────────────────────────────────────────────
    let metrics_name = server_plugin(config, "metrics", "no-metrics-server");
    let metrics_cfg  = server_config(config, "metrics");
    let metrics = {
        let factory = components.iter().find_map(|c| {
            if let PluginComponent::MetricsServer(f) = c {
                if f.name() == metrics_name { Some(f) } else { None }
            } else {
                None
            }
        }).ok_or_else(|| ServerBuildError::FactoryNotFound {
            role: "metrics",
            name: metrics_name.to_string(),
        })?;
        factory.build(metrics_cfg).map_err(|e| ServerBuildError::BuildFailed {
            role: "metrics",
            name: metrics_name.to_string(),
            reason: e.to_string(),
        })?
    };

    // ── REST server ───────────────────────────────────────────────────────
    let rest_name = server_plugin(config, "rest", "axum-rest-server");
    let rest_cfg  = server_config(config, "rest");
    let rest = {
        let factory = components.iter().find_map(|c| {
            if let PluginComponent::RestServer(f) = c {
                if f.name() == rest_name { Some(f) } else { None }
            } else {
                None
            }
        }).ok_or_else(|| ServerBuildError::FactoryNotFound {
            role: "rest",
            name: rest_name.to_string(),
        })?;
        let deps = RestServerDeps { contributors: api_contributors };
        factory.build(rest_cfg, deps).map_err(|e| ServerBuildError::BuildFailed {
            role: "rest",
            name: rest_name.to_string(),
            reason: format!("failed to build REST server: {e}"),
        })?
    };

    Ok(BuiltServers { state, workflow, metrics, rest })
}

// ── Config helpers ─────────────────────────────────────────────────────────

/// Returns the plugin name configured for `servers.<role>.plugin`, or `default`.
fn server_plugin<'a>(config: &'a Value, role: &str, default: &'static str) -> &'a str {
    config
        .get("servers")
        .and_then(|s| s.get(role))
        .and_then(|r| r.get("plugin"))
        .and_then(|v| v.as_str())
        .unwrap_or(default)
}

/// Returns the `servers.<role>.config` sub-object, or an empty object.
fn server_config(config: &Value, role: &str) -> Value {
    config
        .get("servers")
        .and_then(|s| s.get(role))
        .and_then(|r| r.get("config"))
        .cloned()
        .unwrap_or(Value::Object(Default::default()))
}
