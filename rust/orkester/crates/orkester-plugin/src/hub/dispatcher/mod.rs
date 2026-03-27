mod component;
mod drop;

pub use component::{ComponentEntry, ComponentRegistry, ComponentTarget, ComponentsDispatcher};
pub use drop::DropDispatcher;

use crate::hub::{config::TargetConfig, envelope::Envelope, error::{DispatchError, HubError}};

// ── Dispatcher trait ──────────────────────────────────────────────────────────

/// Delivers an [`Envelope`] to one or more targets.
///
/// The router calls `dispatch` on each dispatcher associated with the matched
/// rule.  Implementations must be `Send + Sync`; all dispatchers for a given
/// router run on the router thread.
pub trait Dispatcher: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn dispatch(&self, envelope: Envelope) -> Result<(), DispatchError>;
}

// ── Validation ────────────────────────────────────────────────────────────────

/// Returns `Err` if `cfg.kind` is unknown, without constructing anything.
pub fn validate(cfg: &TargetConfig) -> Result<(), HubError> {
    match cfg.kind.as_str() {
        "components" | "drop" => Ok(()),
        other => Err(HubError::InvalidConfig(format!(
            "unknown target kind '{other}'; supported: components, drop",
        ))),
    }
}

// ── Factory ───────────────────────────────────────────────────────────────────

/// Instantiate a [`Dispatcher`] from config.
///
/// `registry` is threaded through for dispatchers that need to reach ABI
/// components at dispatch time.
pub fn build(cfg: &TargetConfig, registry: ComponentRegistry) -> Result<Box<dyn Dispatcher>, HubError> {
    match cfg.kind.as_str() {
        "components" => {
            let d = ComponentsDispatcher::from_config(&cfg.config, registry)
                .map_err(|e| HubError::InvalidConfig(format!("target 'components': {e}")))?;
            Ok(Box::new(d))
        }
        "drop" => Ok(Box::new(DropDispatcher)),
        other => Err(HubError::InvalidConfig(format!("unknown target kind '{other}'"))),
    }
}
