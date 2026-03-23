mod all;
mod match_filter;

pub use all::AllFilter;
pub use match_filter::MatchFilter;

use crate::hub::{config::FilterConfig, envelope::Envelope, error::HubError};

// ── Trait ─────────────────────────────────────────────────────────────────────

/// Predicate evaluated against an [`Envelope`]'s routing-visible fields.
///
/// Implementations must be thread-safe: the same instance is used by the
/// single router thread but may be arc-cloned for route reloads.
pub trait Filter: Send + Sync + 'static {
    fn matches(&self, envelope: &Envelope) -> bool;
}

// ── Factory ───────────────────────────────────────────────────────────────────

/// Instantiate a [`Filter`] from config.  Returns [`HubError::InvalidConfig`]
/// for unknown kinds or malformed parameters.
pub fn build(cfg: &FilterConfig) -> Result<Box<dyn Filter>, HubError> {
    match cfg.kind.as_str() {
        "all" => Ok(Box::new(AllFilter)),
        "match" => {
            let f = MatchFilter::from_config(&cfg.config)
                .map_err(|e| HubError::InvalidConfig(format!("filter 'match': {e}")))?;
            Ok(Box::new(f))
        }
        other => Err(HubError::InvalidConfig(
            format!("unknown filter kind '{other}'; supported: all, match"),
        )),
    }
}
