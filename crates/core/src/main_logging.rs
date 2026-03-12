use serde_json::Value;

use crate::config::ConfigTree;

use orkester_common::logging::{
    filter::{AllFilter, LogFilter, StrMatch, level_min, source},
    consumers::{ConsoleConsumer, FileConsumer},
    Level, Logger, LogConsumer,
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
///     enabled: true          # default true
///     level:   "INFO"        # TRACE/DEBUG/INFO/WARN/ERROR; absent = no level filter
///     source:  "orkester"    # prefix match on source; absent or empty = no source filter
///   file:
///     enabled: true
///     path:    "/var/log/orkester.log"
///     level:   "WARN"
///     source:  ""            # no source filter
/// ```
pub(crate) fn load_logging_config(config_tree: &ConfigTree) {
    Logger::clear_consumers();
    let mut added = 0usize;

    if let Some(cfg) = config_tree.get("logging.console") {
        if cfg.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true) {
            let consumer = ConsoleConsumer::new();
            consumer.set_filter(build_filter(&cfg));
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
                        consumer.set_filter(build_filter(&cfg));
                        Logger::add_consumer(consumer);
                        added += 1;
                    }
                    Err(e) => eprintln!("[logging] could not open log file '{}': {}", path, e),
                },
            }
        }
    }
}

/// Build a filter from the optional `filter` sub-object of a consumer config block.
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
        (Some(l), None)    => Some(l),
        (None,    Some(s)) => Some(s),
        (None,    None)    => None,
    }
}

fn parse_level(s: &str) -> Option<Level> {
    match s.to_uppercase().as_str() {
        "TRACE" => Some(Level::TRACE),
        "DEBUG" => Some(Level::DEBUG),
        "INFO"  => Some(Level::INFO),
        "WARN"  => Some(Level::WARN),
        "ERROR" => Some(Level::ERROR),
        _       => None,
    }
}
