use std::fmt;

/// Every error that can surface through the Orkester plugin SDK.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Failed to load or link the shared library.
    Library(libloading::Error),

    /// The plugin returned a null component pointer.
    NullComponent,

    /// JSON serialization or deserialization failed.
    Json(serde_json::Error),

    /// Response payload bytes were not valid UTF-8.
    Utf8(std::str::Utf8Error),

    /// A response carried an unexpected payload format code.
    UnexpectedFormat { expected: u32, got: u32 },

    /// A pointer-typed response payload had the wrong byte length.
    InvalidPointerPayload,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Library(e) => write!(f, "failed to load plugin library: {e}"),
            Error::NullComponent => write!(f, "plugin returned a null component pointer"),
            Error::Json(e) => write!(f, "JSON error: {e}"),
            Error::Utf8(e) => write!(f, "payload is not valid UTF-8: {e}"),
            Error::UnexpectedFormat { expected, got } => write!(
                f,
                "unexpected response format (expected {expected}, got {got})"
            ),
            Error::InvalidPointerPayload => write!(
                f,
                "response payload length does not match a component pointer"
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Library(e) => Some(e),
            Error::Json(e) => Some(e),
            Error::Utf8(e) => Some(e),
            _ => None,
        }
    }
}

impl From<libloading::Error> for Error {
    fn from(e: libloading::Error) -> Self {
        Error::Library(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(e: std::str::Utf8Error) -> Self {
        Error::Utf8(e)
    }
}
