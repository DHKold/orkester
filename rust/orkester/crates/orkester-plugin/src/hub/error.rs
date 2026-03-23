use std::fmt;

// ── Hub-level errors ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum HubError {
    /// Route config is invalid.
    InvalidConfig(String),
    /// An operation requires the hub to be running.
    NotRunning,
    /// Hub is already running.
    AlreadyRunning,
    /// A background worker thread failed.
    WorkerFailed(String),
    /// Internal / unexpected error.
    Internal(String),
}

impl fmt::Display for HubError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HubError::InvalidConfig(m) => write!(f, "invalid hub config: {m}"),
            HubError::NotRunning       => write!(f, "hub is not running"),
            HubError::AlreadyRunning   => write!(f, "hub is already running"),
            HubError::WorkerFailed(m)  => write!(f, "hub worker failed: {m}"),
            HubError::Internal(m)      => write!(f, "hub internal error: {m}"),
        }
    }
}

impl std::error::Error for HubError {}

// ── Submission error ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum SubmitError {
    /// The waiting queue is full (Reject backpressure policy).
    QueueFull,
    /// The hub is not running.
    NotRunning,
}

impl fmt::Display for SubmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SubmitError::QueueFull  => write!(f, "hub waiting queue is full"),
            SubmitError::NotRunning => write!(f, "hub is not running"),
        }
    }
}

impl std::error::Error for SubmitError {}

// ── Dispatch error ────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct DispatchError {
    pub dispatcher: String,
    pub cause: String,
}

impl fmt::Display for DispatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dispatcher '{}': {}", self.dispatcher, self.cause)
    }
}

impl std::error::Error for DispatchError {}
