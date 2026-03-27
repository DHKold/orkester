use std::sync::Arc;

use crossbeam_channel::Receiver;

use crate::hub::{
    config::HubConfig,
    dispatcher,
    dispatcher::ComponentRegistry,
    envelope::Envelope,
    error::HubError,
    filter,
    router::{RouteRule, Router},
    stats::HubStats,
};

// ── HubBuilder ────────────────────────────────────────────────────────────────

pub struct HubBuilder {
    config:   HubConfig,
    registry: ComponentRegistry,
}

impl HubBuilder {
    pub fn new(config: HubConfig, registry: ComponentRegistry) -> Self {
        Self { config, registry }
    }

    /// Validate all filter and target kinds without instantiating threads.
    ///
    /// Returns detailed error messages that identify the problematic rule,
    /// filter kind, or target kind by name.
    pub fn validate(&self) -> Result<(), HubError> {
        for (name, rule) in &self.config.routes {
            for f in &rule.filters {
                filter::build(f).map(drop).map_err(|e| {
                    HubError::InvalidConfig(format!(
                        "route '{name}' has invalid filter kind '{}': {e}",
                        f.kind
                    ))
                })?;
            }
            for t in &rule.targets {
                dispatcher::validate(t).map_err(|e| {
                    HubError::InvalidConfig(format!(
                        "route '{name}' has invalid target kind '{}': {e}",
                        t.kind
                    ))
                })?;
            }
        }
        Ok(())
    }

    pub fn build_rules(self) -> Vec<RouteRule> {
        self.config.routes.into_iter().map(|(name, rule_cfg)| {
            let filters = rule_cfg
                .filters
                .into_iter()
                .map(|f| filter::build(&f).expect("validated above"))
                .collect();

            let dispatchers = rule_cfg
                .targets
                .into_iter()
                .map(|t| dispatcher::build(&t, self.registry.clone()).expect("validated above"))
                .collect();

            RouteRule { name, filters, dispatchers }
        }).collect()
    }

    /// Consume the builder and produce a ready-to-run [`Router`].
    ///
    /// Instantiates all filters and dispatchers but does **not** spawn any
    /// threads.  Call [`Router::run`] on a background thread.
    pub fn build_router(
        self,
        rx:    Receiver<Envelope>,
        stats: Arc<HubStats>,
    ) -> Result<Router, HubError> {
        let rules = self.build_rules();
        Ok(Router::new(rx, rules, stats))
    }
}
