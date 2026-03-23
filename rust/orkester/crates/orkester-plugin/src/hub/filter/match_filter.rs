use regex::Regex;
use serde::Deserialize;
use serde_json::Value;

use super::Filter;
use crate::hub::envelope::Envelope;

// ── Config types ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Field { Kind, Owner, Format }

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Mode { Exact, Contains, Regex }

#[derive(Deserialize)]
struct Cfg {
    field: Field,
    value: String,
    #[serde(default = "exact")]
    mode: Mode,
}

fn exact() -> Mode { Mode::Exact }

// ── Filter ────────────────────────────────────────────────────────────────────

/// Matches when a given envelope field satisfies a string condition.
pub struct MatchFilter {
    field: Field,
    value: String,
    mode:  Mode,
    re:    Option<Regex>,
}

impl MatchFilter {
    pub fn from_config(config: &Value) -> Result<Self, String> {
        let cfg: Cfg = serde_json::from_value(config.clone()).map_err(|e| e.to_string())?;
        let re = match &cfg.mode {
            Mode::Regex => Some(Regex::new(&cfg.value).map_err(|e| format!("bad regex: {e}"))?),
            _ => None,
        };
        Ok(Self { field: cfg.field, value: cfg.value, mode: cfg.mode, re })
    }

    fn extract<'a>(&self, env: &'a Envelope) -> Option<&'a str> {
        match self.field {
            Field::Kind   => Some(&env.kind),
            Field::Format => Some(&env.format),
            Field::Owner  => env.owner.as_deref(),
        }
    }
}

impl Filter for MatchFilter {
    fn matches(&self, env: &Envelope) -> bool {
        let Some(v) = self.extract(env) else { return false };
        match &self.mode {
            Mode::Exact    => v == self.value,
            Mode::Contains => v.contains(&self.value),
            Mode::Regex    => self.re.as_ref().map_or(false, |re| re.is_match(v)),
        }
    }
}
