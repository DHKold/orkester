use serde::{Deserialize, Serialize};

/// A numeric log level. Lower values are less severe.
///
/// Five named levels are provided as associated constants, but any `i32`
/// is a valid level, letting callers define domain-specific severity values.
///
/// # Examples
/// ```
/// use crate::logging::level::Level;
///
/// Logger::log(Level::INFO, "hello");
/// Logger::log(Level(25), "between INFO and WARN");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Level(pub i32);

impl Level {
    pub const TRACE: Level = Level(0);
    pub const DEBUG: Level = Level(10);
    pub const INFO: Level = Level(20);
    pub const WARN: Level = Level(30);
    pub const ERROR: Level = Level(40);
}

impl From<i32> for Level {
    fn from(value: i32) -> Self {
        Level(value)
    }
}

impl From<Level> for i32 {
    fn from(level: Level) -> Self {
        level.0
    }
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Level::TRACE => write!(f, "TRACE"),
            Level::DEBUG => write!(f, "DEBUG"),
            Level::INFO => write!(f, "INFO"),
            Level::WARN => write!(f, "WARN"),
            Level::ERROR => write!(f, "ERROR"),
            Level(n) => write!(f, "LEVEL({})", n),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_constants_have_correct_values() {
        assert_eq!(Level::TRACE.0, 0);
        assert_eq!(Level::DEBUG.0, 10);
        assert_eq!(Level::INFO.0, 20);
        assert_eq!(Level::WARN.0, 30);
        assert_eq!(Level::ERROR.0, 40);
    }

    #[test]
    fn ordering_is_numeric() {
        assert!(Level::TRACE < Level::DEBUG);
        assert!(Level::DEBUG < Level::INFO);
        assert!(Level::INFO < Level::WARN);
        assert!(Level::WARN < Level::ERROR);
        assert!(Level(25) > Level::INFO);
        assert!(Level(25) < Level::WARN);
    }

    #[test]
    fn display_named_levels() {
        assert_eq!(Level::TRACE.to_string(), "TRACE");
        assert_eq!(Level::DEBUG.to_string(), "DEBUG");
        assert_eq!(Level::INFO.to_string(), "INFO");
        assert_eq!(Level::WARN.to_string(), "WARN");
        assert_eq!(Level::ERROR.to_string(), "ERROR");
    }

    #[test]
    fn display_custom_level() {
        assert_eq!(Level(25).to_string(), "LEVEL(25)");
        assert_eq!(Level(-1).to_string(), "LEVEL(-1)");
    }

    #[test]
    fn from_i32_round_trips() {
        assert_eq!(Level::from(20), Level::INFO);
        assert_eq!(Level::from(99), Level(99));
    }

    #[test]
    fn into_i32_round_trips() {
        let n: i32 = Level::WARN.into();
        assert_eq!(n, 30);
    }

    #[test]
    fn custom_level_sits_between_named_ones() {
        let between = Level(15);
        assert!(between > Level::DEBUG);
        assert!(between < Level::INFO);
    }
}
