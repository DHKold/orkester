use core::fmt;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Error {
    InvalidMessage,
    InvalidOwnedMessage,
    InvalidUtf8,
    NullOutput,
    NullHostApi,
    HostCallFailed,
    PluginCallFailed,
    Panic,
    Custom(&'static str),
}

pub type Result<T> = core::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMessage => f.write_str("invalid message"),
            Self::InvalidOwnedMessage => f.write_str("invalid owned message"),
            Self::InvalidUtf8 => f.write_str("invalid utf-8"),
            Self::NullOutput => f.write_str("null output pointer"),
            Self::NullHostApi => f.write_str("null host api"),
            Self::HostCallFailed => f.write_str("host call failed"),
            Self::PluginCallFailed => f.write_str("plugin call failed"),
            Self::Panic => f.write_str("panic across ffi boundary"),
            Self::Custom(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {}