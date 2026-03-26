/// Returns `true` — used as `#[serde(default = "default_true")]`.
pub fn default_true() -> bool { true }

/// Returns `false` — used as `#[serde(default = "default_false")]`.
pub fn default_false() -> bool { false }

/// Returns an empty vector — used as `#[serde(default = "default_vec")]`.
pub fn default_vec<T>() -> Vec<T> { Vec::new() }

/// Returns `"UTC"` — used as `#[serde(default = "default_utc")]`.
pub fn default_utc() -> String { "UTC".to_string() }