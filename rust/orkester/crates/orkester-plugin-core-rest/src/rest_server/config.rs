use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct RestServerConfig {
    /// TCP address to bind (default: `127.0.0.1:8080`).
    #[serde(default = "default_bind")]
    pub bind: String,
    /// API routes dispatched to the hub.
    #[serde(default)]
    pub routes: Vec<RouteEntry>,
    /// Directories to serve as static files.
    #[serde(default)]
    pub static_folders: Vec<StaticFolderEntry>,
    /// Individual files to serve at a specific URL path.
    #[serde(default)]
    pub static_files: Vec<StaticFileEntry>,
    /// Allowed CORS origins (empty = permissive / allow all).
    #[serde(default)]
    pub cors_origins: Vec<String>,
    /// Optional TLS certificate path (PEM). TLS is enabled when both fields are set.
    #[serde(default)]
    pub tls_cert: Option<String>,
    /// Optional TLS private key path (PEM).
    #[serde(default)]
    pub tls_key: Option<String>,
}

/// A single API route mapped to a hub action.
#[derive(Debug, Clone, Deserialize)]
pub struct RouteEntry {
    /// URL pattern, e.g. `/v1/workflow/work-runs/{name}`.
    pub path:   String,
    /// HTTP method (`GET`, `POST`, …), or `ANY`.
    pub method: String,
    /// Hub action kind, e.g. `workaholic/WorkflowServer/GetWorkRun`.
    pub action: String,
}

/// Serves static files from `dir` under the `url_path` URL prefix.
/// Missing files fall back to `{dir}/index.html` for SPA support.
#[derive(Debug, Clone, Deserialize)]
pub struct StaticFolderEntry {
    pub url_path: String,
    pub dir:      String,
}

/// Serves a single file at an exact URL path.
#[derive(Debug, Clone, Deserialize)]
pub struct StaticFileEntry {
    /// Exact URL path to serve this file at, e.g. `/favicon.ico`.
    pub url_path: String,
    /// Path to the file on disk, e.g. `/orkester/ui/favicon.ico`.
    pub file:     String,
}

fn default_bind() -> String { "127.0.0.1:8080".into() }
