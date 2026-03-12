//! Filter predicates for log consumers.
//!
//! # Generic primitive filters
//!
//! | Type                    | Passes when                                         |
//! |-------------------------|-----------------------------------------------------|
//! | [`IntMinFilter`]        | extracted integer >= min                            |
//! | [`IntMaxFilter`]        | extracted integer <= max                            |
//! | [`StrMatchesFilter`]    | extracted string satisfies a [`StrMatch`] rule      |
//! | [`StrAnyMatchesFilter`] | any string in extracted list satisfies a [`StrMatch`] rule |
//! | [`DateTimeFilter`]      | timestamp is within an optional time window         |
//!
//! # Combinators
//!
//! | Type          | Semantics   |
//! |---------------|-------------|
//! | [`AllFilter`] | logical AND |
//! | [`AnyFilter`] | logical OR  |
//! | [`NotFilter`] | logical NOT |
//!
//! # Log-field convenience constructors
//!
//! | Function      | Field                   |
//! |---------------|-------------------------|
//! | [`level_min`] | `log.level >= min`      |
//! | [`level_max`] | `log.level <= max`      |
//! | [`source`]    | `log.source` match      |
//! | [`tag`]       | any `log.tags[]` match  |
//!
//! # Example
//!
//! ```no_run
//! use orkester_common::logging::filter::{AllFilter, level_min, source, StrMatch};
//! use orkester_common::logging::{Level, consumers::ConsoleConsumer};
//!
//! let consumer = ConsoleConsumer::new();
//! consumer.set_filter(Some(AllFilter::new(vec![
//!     Box::new(level_min(Level::INFO)),
//!     Box::new(source(StrMatch::Prefix("orkester".into()))),
//! ])));
//! ```

use chrono::{DateTime, Utc};
use regex::Regex;

use crate::logging::{level::Level, log::Log};

// ── Trait ─────────────────────────────────────────────────────────────────────

/// A predicate on a [`Log`] entry.
///
/// Implementations must be `Send + Sync` so they can be used inside
/// consumers that are shared across threads.
pub trait LogFilter: Send + Sync {
    fn matches(&self, log: &Log) -> bool;
}

impl LogFilter for Box<dyn LogFilter> {
    fn matches(&self, log: &Log) -> bool {
        (**self).matches(log)
    }
}

// ── Generic: integer ─────────────────────────────────────────────────────────

/// Passes when the extracted integer is **at or above** `min`.
///
/// Use [`level_min`] for the common case of filtering on `log.level`.
pub struct IntMinFilter {
    field: Box<dyn Fn(&Log) -> i64 + Send + Sync>,
    pub min: i64,
}

impl IntMinFilter {
    pub fn new(field: impl Fn(&Log) -> i64 + Send + Sync + 'static, min: i64) -> Self {
        Self {
            field: Box::new(field),
            min,
        }
    }
}

impl LogFilter for IntMinFilter {
    fn matches(&self, log: &Log) -> bool {
        (self.field)(log) >= self.min
    }
}

/// Passes when the extracted integer is **at or below** `max`.
///
/// Use [`level_max`] for the common case of filtering on `log.level`.
pub struct IntMaxFilter {
    field: Box<dyn Fn(&Log) -> i64 + Send + Sync>,
    pub max: i64,
}

impl IntMaxFilter {
    pub fn new(field: impl Fn(&Log) -> i64 + Send + Sync + 'static, max: i64) -> Self {
        Self {
            field: Box::new(field),
            max,
        }
    }
}

impl LogFilter for IntMaxFilter {
    fn matches(&self, log: &Log) -> bool {
        (self.field)(log) <= self.max
    }
}

// ── Generic: string match ─────────────────────────────────────────────────────

/// How a string value must match.
pub enum StrMatch {
    /// Exact equality (case-sensitive).
    Exact(String),
    /// The value contains the pattern as a substring (case-sensitive).
    Contains(String),
    /// The value starts with the pattern (case-sensitive).
    Prefix(String),
    /// The value ends with the pattern (case-sensitive).
    Suffix(String),
    /// The value matches a compiled regular expression.
    Regex(Regex),
}

impl StrMatch {
    /// Compiles `pattern` as a regular expression.
    ///
    /// Returns an error if the pattern is invalid.
    pub fn regex(pattern: &str) -> Result<Self, regex::Error> {
        Regex::new(pattern).map(StrMatch::Regex)
    }

    fn test(&self, s: &str) -> bool {
        match self {
            StrMatch::Exact(p) => s == p.as_str(),
            StrMatch::Contains(p) => s.contains(p.as_str()),
            StrMatch::Prefix(p) => s.starts_with(p.as_str()),
            StrMatch::Suffix(p) => s.ends_with(p.as_str()),
            StrMatch::Regex(re) => re.is_match(s),
        }
    }
}

/// Passes when a single extracted string satisfies a [`StrMatch`] rule.
///
/// Use [`source`] for the common case of matching on `log.source`.
pub struct StrMatchesFilter {
    field: Box<dyn Fn(&Log) -> String + Send + Sync>,
    pub pattern: StrMatch,
}

impl StrMatchesFilter {
    pub fn new(field: impl Fn(&Log) -> String + Send + Sync + 'static, pattern: StrMatch) -> Self {
        Self {
            field: Box::new(field),
            pattern,
        }
    }
}

impl LogFilter for StrMatchesFilter {
    fn matches(&self, log: &Log) -> bool {
        self.pattern.test(&(self.field)(log))
    }
}

/// Passes when **any** string in the extracted list satisfies a [`StrMatch`] rule.
///
/// Use [`tag`] for the common case of checking `log.tags`.
pub struct StrAnyMatchesFilter {
    field: Box<dyn Fn(&Log) -> Vec<String> + Send + Sync>,
    pub pattern: StrMatch,
}

impl StrAnyMatchesFilter {
    pub fn new(
        field: impl Fn(&Log) -> Vec<String> + Send + Sync + 'static,
        pattern: StrMatch,
    ) -> Self {
        Self {
            field: Box::new(field),
            pattern,
        }
    }
}

impl LogFilter for StrAnyMatchesFilter {
    fn matches(&self, log: &Log) -> bool {
        (self.field)(log).iter().any(|s| self.pattern.test(s))
    }
}

// ── DateTime filter ───────────────────────────────────────────────────────────

/// Passes log entries whose timestamp falls within an optional time window.
///
/// Both bounds are **inclusive**. Leave either bound as `None` to keep
/// that end of the window open.
pub struct DateTimeFilter {
    /// If set, only entries timestamped **at or after** this instant pass.
    pub after: Option<DateTime<Utc>>,
    /// If set, only entries timestamped **at or before** this instant pass.
    pub before: Option<DateTime<Utc>>,
}

impl DateTimeFilter {
    /// Passes entries timestamped at or after `t` (open upper bound).
    pub fn after(t: DateTime<Utc>) -> Self {
        Self {
            after: Some(t),
            before: None,
        }
    }

    /// Passes entries timestamped at or before `t` (open lower bound).
    pub fn before(t: DateTime<Utc>) -> Self {
        Self {
            after: None,
            before: Some(t),
        }
    }

    /// Passes entries timestamped between `after` and `before` (inclusive).
    pub fn between(after: DateTime<Utc>, before: DateTime<Utc>) -> Self {
        Self {
            after: Some(after),
            before: Some(before),
        }
    }
}

impl LogFilter for DateTimeFilter {
    fn matches(&self, log: &Log) -> bool {
        if let Some(after) = self.after {
            if log.datetime < after {
                return false;
            }
        }
        if let Some(before) = self.before {
            if log.datetime > before {
                return false;
            }
        }
        true
    }
}

// ── Combinators ───────────────────────────────────────────────────────────────

/// Passes when **all** inner filters match (logical AND).
///
/// An empty inner set accepts every entry.
pub struct AllFilter {
    filters: Vec<Box<dyn LogFilter>>,
}

impl AllFilter {
    pub fn new(filters: Vec<Box<dyn LogFilter>>) -> Self {
        Self { filters }
    }
}

impl LogFilter for AllFilter {
    fn matches(&self, log: &Log) -> bool {
        self.filters.iter().all(|f| f.matches(log))
    }
}

/// Passes when **any** inner filter matches (logical OR).
///
/// An empty inner set rejects every entry.
pub struct AnyFilter {
    filters: Vec<Box<dyn LogFilter>>,
}

impl AnyFilter {
    pub fn new(filters: Vec<Box<dyn LogFilter>>) -> Self {
        Self { filters }
    }
}

impl LogFilter for AnyFilter {
    fn matches(&self, log: &Log) -> bool {
        self.filters.iter().any(|f| f.matches(log))
    }
}

/// Inverts the result of another filter (logical NOT).
pub struct NotFilter {
    inner: Box<dyn LogFilter>,
}

impl NotFilter {
    pub fn new(inner: impl LogFilter + 'static) -> Self {
        Self {
            inner: Box::new(inner),
        }
    }
}

impl LogFilter for NotFilter {
    fn matches(&self, log: &Log) -> bool {
        !self.inner.matches(log)
    }
}

// ── Log-field convenience constructors ───────────────────────────────────────

/// Passes when `log.level` is **at or above** `min`.
pub fn level_min(min: Level) -> IntMinFilter {
    IntMinFilter::new(|log| log.level.0 as i64, min.0 as i64)
}

/// Passes when `log.level` is **at or below** `max`.
pub fn level_max(max: Level) -> IntMaxFilter {
    IntMaxFilter::new(|log| log.level.0 as i64, max.0 as i64)
}

/// Passes when `log.source` satisfies the given [`StrMatch`] rule.
pub fn source(pattern: StrMatch) -> StrMatchesFilter {
    StrMatchesFilter::new(|log| log.source.clone(), pattern)
}

/// Passes when **any** element of `log.tags` satisfies the given [`StrMatch`] rule.
pub fn tag(pattern: StrMatch) -> StrAnyMatchesFilter {
    StrAnyMatchesFilter::new(|log| log.tags.clone(), pattern)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::{level::Level, log::Log};
    use chrono::{Duration, Utc};

    fn make(level: Level, src: &str, tags: Vec<String>, msg: &str) -> Log {
        Log::new(level, src, tags, msg)
    }

    // ── IntMinFilter / IntMaxFilter ────────────────────────────────────────────

    #[test]
    fn int_min_passes_at_and_above() {
        let f = level_min(Level::INFO);
        assert!(f.matches(&make(Level::INFO, "s", vec![], "m")));
        assert!(f.matches(&make(Level::WARN, "s", vec![], "m")));
        assert!(f.matches(&make(Level::ERROR, "s", vec![], "m")));
        assert!(!f.matches(&make(Level::DEBUG, "s", vec![], "m")));
        assert!(!f.matches(&make(Level::TRACE, "s", vec![], "m")));
    }

    #[test]
    fn int_max_passes_at_and_below() {
        let f = level_max(Level::WARN);
        assert!(f.matches(&make(Level::WARN, "s", vec![], "m")));
        assert!(f.matches(&make(Level::INFO, "s", vec![], "m")));
        assert!(!f.matches(&make(Level::ERROR, "s", vec![], "m")));
    }

    // ── StrMatchesFilter ───────────────────────────────────────────────────────

    #[test]
    fn str_exact_matches_only_exact() {
        let f = source(StrMatch::Exact("auth".into()));
        assert!(f.matches(&make(Level::INFO, "auth", vec![], "m")));
        assert!(!f.matches(&make(Level::INFO, "auth::sub", vec![], "m")));
    }

    #[test]
    fn str_contains_matches_substring() {
        let f = source(StrMatch::Contains("auth".into()));
        assert!(f.matches(&make(Level::INFO, "my::auth::service", vec![], "m")));
        assert!(!f.matches(&make(Level::INFO, "other", vec![], "m")));
    }

    #[test]
    fn str_prefix_matches_start() {
        let f = source(StrMatch::Prefix("orkester::server".into()));
        assert!(f.matches(&make(Level::INFO, "orkester::server::rest", vec![], "m")));
        assert!(!f.matches(&make(Level::INFO, "orkester::plugin", vec![], "m")));
    }

    #[test]
    fn str_suffix_matches_end() {
        let f = source(StrMatch::Suffix("::rest".into()));
        assert!(f.matches(&make(Level::INFO, "orkester::server::rest", vec![], "m")));
        assert!(!f.matches(&make(Level::INFO, "orkester::server::grpc", vec![], "m")));
    }

    #[test]
    fn str_regex_matches_pattern() {
        let f = source(StrMatch::regex(r"^orkester::server::(rest|grpc)$").unwrap());
        assert!(f.matches(&make(Level::INFO, "orkester::server::rest", vec![], "m")));
        assert!(f.matches(&make(Level::INFO, "orkester::server::grpc", vec![], "m")));
        assert!(!f.matches(&make(Level::INFO, "orkester::server::metrics", vec![], "m")));
    }

    // ── StrAnyMatchesFilter ────────────────────────────────────────────────────

    #[test]
    fn str_any_requires_matching_tag() {
        let f = tag(StrMatch::Exact("api".into()));
        assert!(f.matches(&make(Level::INFO, "s", vec!["api".into()], "m")));
        assert!(!f.matches(&make(Level::INFO, "s", vec!["other".into()], "m")));
        assert!(!f.matches(&make(Level::INFO, "s", vec![], "m")));
    }

    // ── DateTimeFilter ─────────────────────────────────────────────────────────

    #[test]
    fn datetime_after_rejects_old_entries() {
        let future = Utc::now() + Duration::hours(1);
        let f = DateTimeFilter::after(future);
        assert!(!f.matches(&make(Level::INFO, "s", vec![], "m")));
    }

    #[test]
    fn datetime_before_accepts_recent_entries() {
        let future = Utc::now() + Duration::hours(1);
        let f = DateTimeFilter::before(future);
        assert!(f.matches(&make(Level::INFO, "s", vec![], "m")));
    }

    #[test]
    fn datetime_between_rejects_outside_window() {
        let past = Utc::now() - Duration::hours(2);
        let also_past = Utc::now() - Duration::hours(1);
        let f = DateTimeFilter::between(past, also_past);
        assert!(!f.matches(&make(Level::INFO, "s", vec![], "m")));
    }

    // ── Combinators ────────────────────────────────────────────────────────────

    #[test]
    fn all_requires_every_filter() {
        let f = AllFilter::new(vec![
            Box::new(level_min(Level::INFO)),
            Box::new(tag(StrMatch::Exact("api".into()))),
        ]);
        assert!(f.matches(&make(Level::INFO, "s", vec!["api".into()], "m")));
        assert!(!f.matches(&make(Level::DEBUG, "s", vec!["api".into()], "m")));
        assert!(!f.matches(&make(Level::INFO, "s", vec![], "m")));
    }

    #[test]
    fn any_requires_at_least_one_filter() {
        let f = AnyFilter::new(vec![
            Box::new(level_min(Level::ERROR)),
            Box::new(tag(StrMatch::Exact("critical".into()))),
        ]);
        assert!(f.matches(&make(Level::ERROR, "s", vec![], "m")));
        assert!(f.matches(&make(Level::INFO, "s", vec!["critical".into()], "m")));
        assert!(!f.matches(&make(Level::INFO, "s", vec![], "m")));
    }

    #[test]
    fn not_inverts_result() {
        let f = NotFilter::new(tag(StrMatch::Exact("noisy".into())));
        assert!(f.matches(&make(Level::INFO, "s", vec![], "m")));
        assert!(!f.matches(&make(Level::INFO, "s", vec!["noisy".into()], "m")));
    }

    #[test]
    fn all_empty_accepts_everything() {
        let f = AllFilter::new(vec![]);
        assert!(f.matches(&make(Level::TRACE, "s", vec![], "m")));
    }

    #[test]
    fn any_empty_rejects_everything() {
        let f = AnyFilter::new(vec![]);
        assert!(!f.matches(&make(Level::ERROR, "s", vec![], "m")));
    }
}
