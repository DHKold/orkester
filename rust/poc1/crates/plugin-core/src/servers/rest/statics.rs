//! Static file serving for the REST server.
//!
//! Each [`StaticsMount`] maps a URL prefix (e.g. `/ui`) to a filesystem
//! directory and enforces strict security invariants before serving any file.
//!
//! # Security guarantees
//!
//! A file is served **only** when **all** of the following hold:
//!
//! 1. The request path starts with the configured `url_prefix`.
//! 2. The relative portion of the path, after percent-decoding, contains no
//!    `..` segment (defense in depth against traversal before canonicalization).
//! 3. [`std::fs::canonicalize`] succeeds — the file must exist and all
//!    symlinks are fully resolved by the OS.
//! 4. The canonicalized absolute path is strictly prefixed by the
//!    canonicalized directory root (catches symlinks escaping the tree).
//! 5. The file extension extracted from the **canonical path** (not the URL)
//!    is in the configured allowlist — blocks tricks such as
//!    `/file.js%00.exe` where the URL extension and the real extension differ.

use std::collections::HashSet;
use std::path::PathBuf;

use axum::{
    body::Body,
    http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::Response,
};
use serde_json::Value;

use orkester_common::{log_debug, log_info, log_warn};

// ── StaticsMount ──────────────────────────────────────────────────────────────

/// A single static-file serving mount parsed from the server config.
pub(super) struct StaticsMount {
    /// URL prefix this mount handles (normalised, no trailing slash), e.g. `/ui`.
    url_prefix: String,
    /// Canonicalized absolute filesystem root established at parse time.
    canonical_dir: PathBuf,
    /// Allowed lowercase file extensions (no leading dot).
    /// When populated, files with any other extension are rejected with 404.
    allowed_extensions: HashSet<String>,
    /// Additional HTTP response headers to attach to every successful file response.
    extra_headers: Vec<(HeaderName, HeaderValue)>,
}

impl StaticsMount {
    // ── Constructor ───────────────────────────────────────────────────────────

    /// Parse all `statics` mount entries from a server configuration block.
    pub(super) fn parse_all(config: &Value) -> Vec<Self> {
        let arr = match config.get("statics").and_then(|v| v.as_array()) {
            Some(a) => a,
            None => return Vec::new(),
        };

        let mut mounts = Vec::new();

        for entry in arr {
            let url_prefix = match entry.get("url_prefix").and_then(|v| v.as_str()) {
                Some(s) => s.trim_end_matches('/').to_string(),
                None => {
                    log_warn!("Statics: entry missing 'url_prefix' — skipping");
                    continue;
                }
            };

            let dir_str = match entry.get("dir").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => {
                    log_warn!("Statics: mount '{}' missing 'dir' — skipping", url_prefix);
                    continue;
                }
            };

            // Canonicalize at parse time so we have a stable reference point.
            // If the directory doesn't exist yet, skip with a clear warning.
            let canonical_dir = match std::path::Path::new(dir_str).canonicalize() {
                Ok(p) => p,
                Err(e) => {
                    log_warn!(
                        "Statics: cannot resolve dir '{}' for mount '{}': {} — skipping",
                        dir_str, url_prefix, e
                    );
                    continue;
                }
            };

            let allowed_extensions = parse_extensions(entry);
            let extra_headers = parse_headers(entry, &url_prefix);

            log_info!(
                "Statics: mount '{}' → '{}' ({} extension(s) allowed, {} custom header(s))",
                url_prefix,
                canonical_dir.display(),
                allowed_extensions.len(),
                extra_headers.len()
            );

            mounts.push(StaticsMount {
                url_prefix,
                canonical_dir,
                allowed_extensions,
                extra_headers,
            });
        }

        mounts
    }

    // ── Request handling ──────────────────────────────────────────────────────

    /// Attempt to serve `url_path` from this mount.
    ///
    /// Returns:
    /// - `Some(Response)` — this mount is responsible for the path, regardless
    ///   of whether the result is a 200, 404, or 500.
    /// - `None` — the path prefix does not match; the caller should try the
    ///   next mount (or fall through to the normal 404 handler).
    pub(super) async fn try_serve(&self, url_path: &str) -> Option<Response> {
        let after_prefix = strip_prefix(url_path, &self.url_prefix)?;
        let rel = after_prefix.trim_start_matches('/');

        // A bare mount root request (e.g. GET /ui) serves index.html.
        let rel = if rel.is_empty() { "index.html" } else { rel };

        let file_path = match self.safe_resolve(rel) {
            Some(p) => p,
            None => {
                log_debug!(
                    "Statics: blocked '{}' — path escape attempt or file does not exist",
                    url_path
                );
                return Some(not_found());
            }
        };

        // Extension check on the CANONICAL path, not the URL.
        // This prevents tricks like `/file.js%00.exe` (null-byte injection)
        // or serving a symlink whose real extension differs.
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        if !self.allowed_extensions.is_empty() && !self.allowed_extensions.contains(&ext) {
            log_debug!(
                "Statics: blocked '{}' — extension '.{}' is not in allowlist",
                url_path, ext
            );
            return Some(not_found());
        }

        match tokio::fs::read(&file_path).await {
            Ok(bytes) => {
                log_debug!(
                    "Statics: 200 '{}' ({} bytes, type={})",
                    file_path.display(),
                    bytes.len(),
                    mime_type(&ext)
                );
                Some(build_response(bytes, &ext, &self.extra_headers))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                log_debug!("Statics: 404 — file not found at '{}'", file_path.display());
                Some(not_found())
            }
            Err(e) => {
                log_warn!(
                    "Statics: I/O error reading '{}': {}",
                    file_path.display(),
                    e
                );
                Some(server_error())
            }
        }
    }

    // ── Path resolution ───────────────────────────────────────────────────────

    /// Resolve `rel` to a canonical absolute path strictly inside
    /// `self.canonical_dir`.
    ///
    /// Returns `None` when:
    /// - The relative path contains a `..` component (traversal).
    /// - The path cannot be percent-decoded as UTF-8 (malformed URL).
    /// - The file does not exist (`canonicalize` fails).
    /// - The canonical path escapes `canonical_dir` (symlink escape).
    fn safe_resolve(&self, rel: &str) -> Option<PathBuf> {
        let safe_rel = safe_relative_path(rel)?;
        let candidate = self.canonical_dir.join(&safe_rel);

        // canonicalize() resolves ALL symlinks and `..` components and returns
        // Err if the file does not exist — this is the primary security gate.
        let canonical = candidate.canonicalize().ok()?;

        // SECURITY: the fully resolved path must be strictly inside our root.
        // This is the catch-all that defeats symlink escapes.
        if !canonical.starts_with(&self.canonical_dir) {
            log_warn!(
                "Statics: path escape blocked — '{}' is outside '{}'",
                canonical.display(),
                self.canonical_dir.display()
            );
            return None;
        }

        Some(canonical)
    }
}

// ── Config parsing ────────────────────────────────────────────────────────────

fn parse_extensions(entry: &Value) -> HashSet<String> {
    let mut set = HashSet::new();
    let filters = match entry.get("filters").and_then(|v| v.as_array()) {
        Some(f) => f,
        None => return set,
    };
    for filter in filters {
        let exts = match filter.get("extension").and_then(|v| v.as_array()) {
            Some(e) => e,
            None => continue,
        };
        for ext in exts {
            if let Some(e) = ext.as_str() {
                // Normalise: strip any leading dot, lowercase.
                set.insert(e.trim_start_matches('.').to_lowercase());
            }
        }
    }
    set
}

fn parse_headers(entry: &Value, url_prefix: &str) -> Vec<(HeaderName, HeaderValue)> {
    let mut result = Vec::new();
    let headers = match entry.get("headers").and_then(|v| v.as_object()) {
        Some(h) => h,
        None => return result,
    };
    for (k, v) in headers {
        let val_str = match v.as_str() {
            Some(s) => s,
            None => continue,
        };
        match (
            HeaderName::try_from(k.as_str()),
            HeaderValue::try_from(val_str),
        ) {
            (Ok(name), Ok(value)) => result.push((name, value)),
            _ => log_warn!(
                "Statics: invalid header '{}' for mount '{}' — skipping",
                k,
                url_prefix
            ),
        }
    }
    result
}

// ── Path helpers ──────────────────────────────────────────────────────────────

/// Return the portion of `path` after `prefix`, or `None` if `path` does not
/// start with `prefix` followed by `/` (or is not an exact match).
///
/// Prevents prefix `/ui` from matching `/uiother`.
fn strip_prefix<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    if path == prefix {
        return Some(""); // exact match, no trailing path
    }
    if path.starts_with(prefix) && path[prefix.len()..].starts_with('/') {
        return Some(&path[prefix.len()..]);
    }
    None
}

/// Build a safe [`PathBuf`] from a percent-encoded relative URL path.
///
/// - Percent-decodes the entire input.
/// - Skips empty segments and `.` (current-dir references).
/// - Returns `None` immediately on any `..` segment (traversal attempt).
/// - Returns `None` if the result is an empty path.
fn safe_relative_path(rel: &str) -> Option<PathBuf> {
    let decoded = percent_decode_path(rel)?;
    let mut buf = PathBuf::new();
    for seg in decoded.split('/') {
        match seg {
            "" | "." => {} // skip
            ".." => {
                // Traversal attempt — reject the entire path unconditionally.
                return None;
            }
            s => buf.push(s),
        }
    }
    if buf.as_os_str().is_empty() {
        None
    } else {
        Some(buf)
    }
}

/// Percent-decode a URL path string.
///
/// Returns `None` if any `%XX` sequence cannot be interpreted or if the
/// decoded bytes are not valid UTF-8 (malformed encoding → reject).
fn percent_decode_path(s: &str) -> Option<String> {
    let src = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(src.len());
    let mut i = 0;
    while i < src.len() {
        if src[i] == b'%' && i + 2 < src.len() {
            if let (Some(hi), Some(lo)) = (from_hex(src[i + 1]), from_hex(src[i + 2])) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
            // Malformed percent sequence — reject.
            return None;
        }
        out.push(src[i]);
        i += 1;
    }
    String::from_utf8(out).ok()
}

fn from_hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

// ── Response builders ─────────────────────────────────────────────────────────

fn build_response(
    bytes: Vec<u8>,
    ext: &str,
    extra_headers: &[(HeaderName, HeaderValue)],
) -> Response {
    let mut map = HeaderMap::new();
    // mime_type always returns a statically-valid header value — unwrap is safe.
    map.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(mime_type(ext)),
    );
    for (name, value) in extra_headers {
        map.insert(name.clone(), value.clone());
    }
    let mut resp = Response::new(Body::from(bytes));
    *resp.headers_mut() = map;
    resp
}

fn not_found() -> Response {
    let mut resp = Response::new(Body::from("Not Found"));
    *resp.status_mut() = StatusCode::NOT_FOUND;
    resp
}

fn server_error() -> Response {
    let mut resp = Response::new(Body::from("Internal Server Error"));
    *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
    resp
}

/// Map a lowercase file extension to a MIME type string.
/// All returned values are valid `HeaderValue` static strings.
fn mime_type(ext: &str) -> &'static str {
    match ext {
        "html" | "htm" => "text/html; charset=utf-8",
        "css"          => "text/css; charset=utf-8",
        "js" | "mjs"   => "application/javascript; charset=utf-8",
        "json"         => "application/json; charset=utf-8",
        "svg"          => "image/svg+xml",
        "png"          => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif"          => "image/gif",
        "ico"          => "image/x-icon",
        "woff"         => "font/woff",
        "woff2"        => "font/woff2",
        "txt"          => "text/plain; charset=utf-8",
        "xml"          => "application/xml",
        "webp"         => "image/webp",
        _              => "application/octet-stream",
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── strip_prefix ──────────────────────────────────────────────────────────

    #[test]
    fn strip_prefix_exact_match() {
        assert_eq!(strip_prefix("/ui", "/ui"), Some(""));
    }

    #[test]
    fn strip_prefix_with_subpath() {
        assert_eq!(strip_prefix("/ui/src/app.js", "/ui"), Some("/src/app.js"));
    }

    #[test]
    fn strip_prefix_no_match() {
        assert_eq!(strip_prefix("/uiother/file", "/ui"), None);
        assert_eq!(strip_prefix("/api/foo", "/ui"), None);
    }

    #[test]
    fn strip_prefix_trailing_slash_in_path() {
        assert_eq!(strip_prefix("/ui/", "/ui"), Some("/"));
    }

    // ── percent_decode_path ───────────────────────────────────────────────────

    #[test]
    fn decode_plain_ascii() {
        assert_eq!(percent_decode_path("src/app.js"), Some("src/app.js".into()));
    }

    #[test]
    fn decode_percent_encoded_dot() {
        // %2E is '.', should decode
        assert_eq!(percent_decode_path("src%2Fapp.js"), Some("src/app.js".into()));
    }

    #[test]
    fn decode_rejects_malformed_sequence() {
        assert_eq!(percent_decode_path("file%GGname"), None);
    }

    #[test]
    fn decode_rejects_non_utf8() {
        // %80 is a lone continuation byte — not valid UTF-8
        assert_eq!(percent_decode_path("file%80.js"), None);
    }

    // ── safe_relative_path ────────────────────────────────────────────────────

    #[test]
    fn safe_path_normal() {
        let p = safe_relative_path("src/app.js").unwrap();
        assert_eq!(p, PathBuf::from("src/app.js"));
    }

    #[test]
    fn safe_path_rejects_dotdot() {
        assert!(safe_relative_path("../etc/passwd").is_none());
        assert!(safe_relative_path("src/../../etc/passwd").is_none());
    }

    #[test]
    fn safe_path_rejects_encoded_dotdot() {
        // %2E%2E decodes to '..'
        assert!(safe_relative_path("%2E%2E/etc/passwd").is_none());
    }

    #[test]
    fn safe_path_skips_empty_segments_and_dot() {
        let p = safe_relative_path("//src/./app.js").unwrap();
        assert_eq!(p, PathBuf::from("src/app.js"));
    }

    #[test]
    fn safe_path_empty_input_returns_none() {
        assert!(safe_relative_path("").is_none());
        assert!(safe_relative_path("///").is_none());
    }
}
