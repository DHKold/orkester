//! S3 HTTP client: list objects and fetch object content.

use chrono::Utc;

use super::auth::authorization_header;
use super::types::S3LoaderEntryConfig;

pub type S3Result<T> = std::result::Result<T, String>;

// ─── URL helpers ──────────────────────────────────────────────────────────────

fn endpoint(cfg: &S3LoaderEntryConfig) -> String {
    cfg.endpoint_url.clone()
        .unwrap_or_else(|| format!("https://s3.{}.amazonaws.com", cfg.region))
}

fn host(cfg: &S3LoaderEntryConfig) -> String {
    if let Some(ep) = &cfg.endpoint_url {
        ep.trim_start_matches("https://").trim_start_matches("http://").to_string()
    } else {
        format!("s3.{}.amazonaws.com", cfg.region)
    }
}

// ─── Authenticated GET ─────────────────────────────────────────────────────────

fn s3_get(cfg: &S3LoaderEntryConfig, path: &str, query: &str) -> S3Result<String> {
    let now      = Utc::now();
    let datetime = now.format("%Y%m%dT%H%M%SZ").to_string();
    let date     = now.format("%Y%m%d").to_string();
    let url      = if query.is_empty() { format!("{}/{}{}", endpoint(cfg), cfg.bucket, path) }
                   else                { format!("{}/{}{path}?{query}", endpoint(cfg), cfg.bucket) };
    let req = ureq::get(&url).set("x-amz-date", &datetime);
    let req = if let (Some(ak), Some(sk)) = (&cfg.access_key_id, &cfg.secret_access_key) {
        let auth = authorization_header("GET", &format!("/{}{path}", cfg.bucket), query, &host(cfg), &datetime, &date, &cfg.region, ak, sk);
        req.set("Authorization", &auth)
    } else { req };
    req.call().map_err(|e| e.to_string())?.into_string().map_err(|e| e.to_string())
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// List objects under the entry's prefix. Returns `(key, etag)` pairs.
pub fn list_objects(cfg: &S3LoaderEntryConfig) -> S3Result<Vec<(String, String)>> {
    let prefix  = if cfg.recursive { cfg.prefix.clone() } else { cfg.prefix.clone() };
    let delim   = if cfg.recursive { "" } else { "&delimiter=%2F" };
    let query   = format!("list-type=2&prefix={}{delim}", urlenc(&prefix));
    let xml     = s3_get(cfg, "", &query)?;
    Ok(parse_list_xml(&xml))
}

/// Fetch raw bytes of one S3 object by key.
pub fn get_object(cfg: &S3LoaderEntryConfig, key: &str) -> S3Result<Vec<u8>> {
    let path = format!("/{key}");
    let now      = Utc::now();
    let datetime = now.format("%Y%m%dT%H%M%SZ").to_string();
    let date     = now.format("%Y%m%d").to_string();
    let url      = format!("{}/{}{}", endpoint(cfg), cfg.bucket, path);
    let req = ureq::get(&url).set("x-amz-date", &datetime);
    let req = if let (Some(ak), Some(sk)) = (&cfg.access_key_id, &cfg.secret_access_key) {
        let auth = authorization_header("GET", &format!("/{}{path}", cfg.bucket), "", &host(cfg), &datetime, &date, &cfg.region, ak, sk);
        req.set("Authorization", &auth)
    } else { req };
    let mut bytes = Vec::new();
    req.call().map_err(|e| e.to_string())?.into_reader().read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

// ─── XML parsing ──────────────────────────────────────────────────────────────

fn extract_tags(xml: &str, tag: &str) -> Vec<String> {
    let open = format!("<{tag}>"); let close = format!("</{tag}>");
    let mut result = Vec::new(); let mut pos = 0;
    while let Some(s) = xml[pos..].find(&open) {
        let s = pos + s + open.len();
        if let Some(e) = xml[s..].find(&close) { result.push(xml[s..s+e].to_string()); pos = s + e + close.len(); }
        else { break; }
    }
    result
}

fn parse_list_xml(xml: &str) -> Vec<(String, String)> {
    let keys  = extract_tags(xml, "Key");
    let etags = extract_tags(xml, "ETag");
    keys.into_iter().zip(etags).map(|(k, e)| (k, e.trim_matches('"').to_string())).collect()
}

fn urlenc(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric() || "/-_.~".contains(c) { c.to_string() } else { format!("%{:02X}", c as u32) }).collect()
}
