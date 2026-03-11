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
