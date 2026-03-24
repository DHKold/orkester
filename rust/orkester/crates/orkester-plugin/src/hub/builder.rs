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

    /// Consume the builder and produce a ready-to-run [`Router`].
    ///
    /// Instantiates all filters and dispatchers but does **not** spawn any
    /// threads.  Call [`Router::run`] on a background thread.
    pub fn build_router(
        self,
        rx:    Receiver<Envelope>,
        stats: Arc<HubStats>,
    ) -> Result<Router, HubError> {
        let rules_cfg = self.config.routes;
        let registry  = self.registry;

        let mut rules = Vec::with_capacity(rules_cfg.len());
        for (name, rule_cfg) in rules_cfg {
            let filters = rule_cfg
                .filters
                .iter()
                .map(|f| {
                    filter::build(f).map_err(|e| {
                        HubError::InvalidConfig(format!(
                            "route '{name}' filter '{}': {e}",
                            f.kind
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            let dispatchers = rule_cfg
                .targets
                .iter()
                .map(|t| {
                    dispatcher::build(t, registry.clone()).map_err(|e| {
                        HubError::InvalidConfig(format!(
                            "route '{name}' target '{}': {e}",
                            t.kind
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            rules.push(RouteRule {
                name,
                filters,
                dispatchers,
            });
        }

        Ok(Router::new(rx, rules, stats))
    }
}
