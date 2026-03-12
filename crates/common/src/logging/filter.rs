//! Filter predicates for log consumers.
//!
//! A [`LogFilter`] is a boolean predicate on a [`Log`] entry.  Consumers hold
//! a list of filters and accept a log only when **all** filters return `true`.
//!
//! # Primitive filters
//!
//! | Type               | Passes when                                         |
//! |--------------------|-----------------------------------------------------|
//! | [`MinLevel`]       | `log.level >= min`                                  |
//! | [`MaxLevel`]       | `log.level <= max`                                  |
//! | [`SourceFilter`]   | source matches an exact, contains, or prefix rule   |
//! | [`TagFilter`]      | the entry carries a specific tag                    |
//! | [`DateTimeFilter`] | timestamp is within an optional time window         |
//!
//! # Combinators
//!
//! | Type             | Semantics   |
//! |------------------|-------------|
//! | [`AllFilter`]    | logical AND |
//! | [`AnyFilter`]    | logical OR  |
//! | [`NotFilter`]    | logical NOT |
//!
//! # Example
//!
//! ```no_run
//! use orkester_common::logging::filter::{AllFilter, MinLevel, TagFilter};
//! use orkester_common::logging::{Level, consumers::ConsoleConsumer};
//!
//! let consumer = ConsoleConsumer::new()
//!     .with_filter(AllFilter::new(vec![
//!         Box::new(MinLevel::new(Level::INFO)),
//!         Box::new(TagFilter::new("api")),
//!     ]));
//! ```

use chrono::{DateTime, Utc};

use crate::logging::{level::Level, log::Log};

// ── Trait ─────────────────────────────────────────────────────────────────────

/// A predicate on a [`Log`] entry.
///
/// Implementations must be `Send + Sync` so they can be used inside
/// consumers that are shared across threads.
pub trait LogFilter: Send + Sync {
    fn matches(&self, log: &Log) -> bool;
}

// ── Level filters ─────────────────────────────────────────────────────────────

/// Passes log entries whose level is **at or above** `min`.
///
/// Use this to suppress TRACE / DEBUG noise in production consumers.
pub struct MinLevel {
    pub min: Level,
}

impl MinLevel {
    pub fn new(min: Level) -> Self {
        Self { min }
    }
}

impl LogFilter for MinLevel {
    fn matches(&self, log: &Log) -> bool {
        log.level >= self.min
    }
}

/// Passes log entries whose level is **at or below** `max`.
pub struct MaxLevel {
    pub max: Level,
}

impl MaxLevel {
    pub fn new(max: Level) -> Self {
        Self { max }
    }
}

impl LogFilter for MaxLevel {
    fn matches(&self, log: &Log) -> bool {
        log.level <= self.max
    }
}

// ── Source filter ─────────────────────────────────────────────────────────────

/// Specifies how a log entry's `source` string must match.
pub enum SourceMatch {
    /// The source must equal `s` exactly (case-sensitive).
    Exact(String),
    /// The source must contain `s` as a substring (case-sensitive).
    Contains(String),
    /// The source must start with `s` (case-sensitive).
    Prefix(String),
}

/// Passes log entries whose `source` satisfies a [`SourceMatch`] rule.
pub struct SourceFilter {
    pub pattern: SourceMatch,
}

impl SourceFilter {
    /// Passes only entries whose source equals `s` exactly.
    pub fn exact(s: impl Into<String>) -> Self {
        Self {
            pattern: SourceMatch::Exact(s.into()),
        }
    }

    /// Passes entries whose source contains `s` as a substring.
    pub fn contains(s: impl Into<String>) -> Self {
        Self {
            pattern: SourceMatch::Contains(s.into()),
        }
    }

    /// Passes entries whose source starts with `s`.
    pub fn prefix(s: impl Into<String>) -> Self {
        Self {
            pattern: SourceMatch::Prefix(s.into()),
        }
    }
}

impl LogFilter for SourceFilter {
    fn matches(&self, log: &Log) -> bool {
        match &self.pattern {
            SourceMatch::Exact(s) => &log.source == s,
            SourceMatch::Contains(s) => log.source.contains(s.as_str()),
            SourceMatch::Prefix(s) => log.source.starts_with(s.as_str()),
        }
    }
}

// ── Tag filter ────────────────────────────────────────────────────────────────

/// Passes log entries that carry a specific tag (exact, case-sensitive match).
pub struct TagFilter {
    pub tag: String,
}

impl TagFilter {
    pub fn new(tag: impl Into<String>) -> Self {
        Self { tag: tag.into() }
    }
}

impl LogFilter for TagFilter {
    fn matches(&self, log: &Log) -> bool {
        log.tags.iter().any(|t| t == &self.tag)
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::{level::Level, log::Log};
    use chrono::{Duration, Utc};

    fn make(level: Level, source: &str, tags: Vec<String>, msg: &str) -> Log {
        Log::new(level, source, tags, msg)
    }

    // ── Level ──────────────────────────────────────────────────────────────────

    #[test]
    fn min_level_passes_at_and_above() {
        let f = MinLevel::new(Level::INFO);
        assert!(f.matches(&make(Level::INFO, "s", vec![], "m")));
        assert!(f.matches(&make(Level::WARN, "s", vec![], "m")));
        assert!(f.matches(&make(Level::ERROR, "s", vec![], "m")));
        assert!(!f.matches(&make(Level::DEBUG, "s", vec![], "m")));
        assert!(!f.matches(&make(Level::TRACE, "s", vec![], "m")));
    }

    #[test]
    fn max_level_passes_at_and_below() {
        let f = MaxLevel::new(Level::WARN);
        assert!(f.matches(&make(Level::WARN, "s", vec![], "m")));
        assert!(f.matches(&make(Level::INFO, "s", vec![], "m")));
        assert!(!f.matches(&make(Level::ERROR, "s", vec![], "m")));
    }

    // ── Source ─────────────────────────────────────────────────────────────────

    #[test]
    fn source_exact_matches_only_exact() {
        let f = SourceFilter::exact("auth");
        assert!(f.matches(&make(Level::INFO, "auth", vec![], "m")));
        assert!(!f.matches(&make(Level::INFO, "auth::sub", vec![], "m")));
    }

    #[test]
    fn source_contains_matches_substring() {
        let f = SourceFilter::contains("auth");
        assert!(f.matches(&make(Level::INFO, "my::auth::service", vec![], "m")));
        assert!(!f.matches(&make(Level::INFO, "other", vec![], "m")));
    }

    #[test]
    fn source_prefix_matches_start() {
        let f = SourceFilter::prefix("orkester::server");
        assert!(f.matches(&make(Level::INFO, "orkester::server::rest", vec![], "m")));
        assert!(!f.matches(&make(Level::INFO, "orkester::plugin", vec![], "m")));
    }

    // ── Tags ───────────────────────────────────────────────────────────────────

    #[test]
    fn tag_filter_requires_tag_presence() {
        let f = TagFilter::new("api");
        assert!(f.matches(&make(Level::INFO, "s", vec!["api".into()], "m")));
        assert!(!f.matches(&make(Level::INFO, "s", vec!["other".into()], "m")));
        assert!(!f.matches(&make(Level::INFO, "s", vec![], "m")));
    }

    // ── DateTime ───────────────────────────────────────────────────────────────

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
        // A freshly-created log is after `also_past`.
        assert!(!f.matches(&make(Level::INFO, "s", vec![], "m")));
    }

    // ── Combinators ────────────────────────────────────────────────────────────

    #[test]
    fn all_requires_every_filter() {
        let f = AllFilter::new(vec![
            Box::new(MinLevel::new(Level::INFO)),
            Box::new(TagFilter::new("api")),
        ]);
        assert!(f.matches(&make(Level::INFO, "s", vec!["api".into()], "m")));
        assert!(!f.matches(&make(Level::DEBUG, "s", vec!["api".into()], "m")));
        assert!(!f.matches(&make(Level::INFO, "s", vec![], "m")));
    }

    #[test]
    fn any_requires_at_least_one_filter() {
        let f = AnyFilter::new(vec![
            Box::new(MinLevel::new(Level::ERROR)),
            Box::new(TagFilter::new("critical")),
        ]);
        assert!(f.matches(&make(Level::ERROR, "s", vec![], "m")));
        assert!(f.matches(&make(Level::INFO, "s", vec!["critical".into()], "m")));
        assert!(!f.matches(&make(Level::INFO, "s", vec![], "m")));
    }

    #[test]
    fn not_inverts_result() {
        let f = NotFilter::new(TagFilter::new("noisy"));
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
