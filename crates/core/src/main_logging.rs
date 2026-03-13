use serde_json::Value;

use crate::config::ConfigTree;

use orkester_common::logging::{
    consumers::{ConsoleConsumer, FileConsumer},
    filter::{level_min, source, AllFilter, FilterChain, FilterRule, LogFilter, StrMatch},
    Level, LogConsumer, Logger,
};

pub(crate) fn init_logging() {
    let consumer = ConsoleConsumer::new();
    consumer.set_filter(Some(Box::new(level_min(Level::DEBUG))));
    Logger::add_consumer(consumer);
}

/// Reconfigure logging from the config tree.
///
/// Clears all existing consumers, then creates new ones based on
/// `logging.console` and `logging.file` config blocks.  If neither block
/// is present (or both are disabled), a default console consumer at DEBUG+
/// is restored so logs are never silently dropped.
///
/// # Config shape
/// ```yaml
/// logging:
///   console:
///     enabled: true
///     # Ordered filter chain (last-match-wins).  Each rule has an optional
///     # `source` prefix/regex and an optional minimum `level`.  The verdict
///     # of the last rule whose source selector matches the entry is used;
///     # if no rule matches the default is to accept.
///     filters:
///       - level: "DEBUG"                       # baseline: DEBUG+
///       - source: "noisy::module"
///         level: "WARN"                        # override: WARN+ from noisy
///   file:
///     enabled: true
///     path:    "/var/log/orkester.log"
///     filters:
///       - level: "WARN"
/// ```
///
/// A single-object `filter:` key is also accepted for backward compatibility:
/// ```yaml
///     filter:
///       level: "INFO"
///       source: "orkester"
/// ```
pub(crate) fn load_logging_config(config_tree: &ConfigTree) {
    Logger::clear_consumers();
    let mut added = 0usize;

    if let Some(cfg) = config_tree.get("logging.console") {
        if cfg.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true) {
            let consumer = ConsoleConsumer::new();
            consumer.set_filter(build_consumer_filter(&cfg));
            Logger::add_consumer(consumer);
            added += 1;
        }
    }

    if let Some(cfg) = config_tree.get("logging.file") {
        if cfg.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true) {
            match cfg.get("path").and_then(|v| v.as_str()).unwrap_or("") {
                "" => eprintln!("[logging] file consumer configured but `path` is missing"),
                path => match FileConsumer::open(path) {
                    Ok(consumer) => {
                        consumer.set_filter(build_consumer_filter(&cfg));
                        Logger::add_consumer(consumer);
                        added += 1;
                    }
                    Err(e) => eprintln!("[logging] could not open log file '{}': {}", path, e),
                },
            }
        }
    }
}

/// Resolve the filter for a consumer config block.
///
/// Tries `filters:` (ordered array) first; falls back to the legacy
/// single-object `filter:` key.  Returns `None` if neither is present.
fn build_consumer_filter(cfg: &Value) -> Option<Box<dyn LogFilter>> {
    if let Some(arr) = cfg.get("filters").and_then(|v| v.as_array()) {
        return Some(Box::new(build_filter_chain(arr)));
    }
    build_filter(cfg)
}

/// Build a [`FilterChain`] from a YAML array of rule objects.
///
/// Each item may have:
/// - `source`: prefix/regex matched against `log.source`
/// - `level`:  minimum level string (TRACE/DEBUG/INFO/WARN/ERROR)
fn build_filter_chain(arr: &[Value]) -> FilterChain {
    let rules = arr
        .iter()
        .map(|item| {
            let source = item
                .get("source")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| {
                    StrMatch::regex(s).unwrap_or_else(|e| {
                        eprintln!("[logging] invalid source regex '{}': {}", s, e);
                        StrMatch::Prefix(s.to_string())
                    })
                });
            let level = item.get("level").and_then(|v| v.as_str()).and_then(parse_level);
            FilterRule::new(source, level)
        })
        .collect();
    FilterChain::new(rules)
}

/// Build a filter from the optional legacy `filter:` sub-object of a consumer config block.
fn build_filter(cfg: &Value) -> Option<Box<dyn LogFilter>> {
    let filter_cfg = cfg.get("filter")?;

    let level_f = filter_cfg
        .get("level")
        .and_then(|v| v.as_str())
        .and_then(parse_level)
        .map(|l| Box::new(level_min(l)) as Box<dyn LogFilter>);

    let source_f = filter_cfg
        .get("source")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| {
            let pattern = StrMatch::regex(s).unwrap_or_else(|e| {
                eprintln!("[logging] invalid source regex '{}': {}", s, e);
                StrMatch::Prefix(s.to_string())
            });
            Box::new(source(pattern)) as Box<dyn LogFilter>
        });

    match (level_f, source_f) {
        (Some(l), Some(s)) => Some(Box::new(AllFilter::new(vec![l, s]))),
        (Some(l), None) => Some(l),
        (None, Some(s)) => Some(s),
        (None, None) => None,
    }
}

fn parse_level(s: &str) -> Option<Level> {
    match s.to_uppercase().as_str() {
        "TRACE" => Some(Level::TRACE),
        "DEBUG" => Some(Level::DEBUG),
        "INFO" => Some(Level::INFO),
        "WARN" => Some(Level::WARN),
        "ERROR" => Some(Level::ERROR),
        _ => None,
    }
}
