//! AWS credential resolution for the S3 loader.
//!
//! Credentials are resolved in the following priority order:
//! 1. **Static** — `access_key_id` + `secret_access_key` set in `S3LoaderEntryConfig`.
//!    Used for local development, MinIO, or when explicit keys are required.
//! 2. **IRSA** — `AWS_WEB_IDENTITY_TOKEN_FILE` + `AWS_ROLE_ARN` environment variables
//!    are present (injected by EKS for pods with an annotated ServiceAccount).
//!    Calls STS `AssumeRoleWithWebIdentity` and caches the resulting short-lived
//!    credentials, refreshing them before expiry.
//! 3. **Unsigned** — no credentials; requests are sent without an `Authorization`
//!    header (suitable for LocalStack / MinIO without auth, or public buckets).

use chrono::{DateTime, Duration, Utc};
use orkester_plugin::log_error;

use super::types::S3LoaderEntryConfig;

// ─── Credential value ─────────────────────────────────────────────────────────

/// A resolved set of AWS credentials.
#[derive(Clone)]
pub(super) struct AwsCredentials {
    pub access_key_id:     String,
    pub secret_access_key: String,
    /// Present for STS-issued temporary credentials.
    /// Must be sent as `x-amz-security-token` and included in SigV4 signed headers.
    pub session_token:     Option<String>,
}

// ─── Provider ────────────────────────────────────────────────────────────────

/// Resolves and caches AWS credentials for one S3 entry.
///
/// Constructed once per entry via [`CredentialsProvider::from_config`].
/// Thread-safe when held behind `Arc<Mutex<S3Entry>>`.
pub(super) enum CredentialsProvider {
    /// Long-lived static keys supplied in the entry config.
    Static(AwsCredentials),

    /// EKS IRSA: exchange the projected service-account token for short-lived
    /// STS credentials and cache them until `REFRESH_MARGIN_SECS` before expiry.
    Irsa {
        role_arn:   String,
        token_file: String,
        region:     String,
        cached:     Option<CachedCredentials>,
    },

    /// No credentials — requests are sent unsigned.
    Unsigned,
}

pub(super) struct CachedCredentials {
    creds:      AwsCredentials,
    expires_at: DateTime<Utc>,
}

/// Refresh STS credentials this many seconds before they actually expire,
/// to avoid race conditions on long-running scans.
const REFRESH_MARGIN_SECS: i64 = 300;

// ─── Construction ─────────────────────────────────────────────────────────────

impl CredentialsProvider {
    /// Build a provider from an entry config.
    ///
    /// Resolution order:
    /// 1. `access_key_id` + `secret_access_key` both set → [`CredentialsProvider::Static`]
    /// 2. `AWS_WEB_IDENTITY_TOKEN_FILE` + `AWS_ROLE_ARN` env vars present → [`CredentialsProvider::Irsa`]
    /// 3. Neither → [`CredentialsProvider::Unsigned`]
    pub fn from_config(cfg: &S3LoaderEntryConfig) -> Self {
        if let (Some(ak), Some(sk)) = (&cfg.access_key_id, &cfg.secret_access_key) {
            return Self::Static(AwsCredentials {
                access_key_id:     ak.clone(),
                secret_access_key: sk.clone(),
                session_token:     None,
            });
        }
        let token_file = std::env::var("AWS_WEB_IDENTITY_TOKEN_FILE").ok();
        let role_arn   = std::env::var("AWS_ROLE_ARN").ok();
        if let (Some(tf), Some(ra)) = (token_file, role_arn) {
            return Self::Irsa {
                role_arn:   ra,
                token_file: tf,
                region:     cfg.region.clone(),
                cached:     None,
            };
        }
        Self::Unsigned
    }
}

// ─── Resolution ───────────────────────────────────────────────────────────────

impl CredentialsProvider {
    /// Return current credentials, refreshing STS credentials when necessary.
    ///
    /// Returns `None` for unsigned entries. Logs an error (and returns stale
    /// cached credentials when available) if the STS refresh call fails.
    pub fn resolve(&mut self) -> Option<AwsCredentials> {
        match self {
            Self::Unsigned  => None,
            Self::Static(c) => Some(c.clone()),
            Self::Irsa { role_arn, token_file, region, cached } => {
                let refresh_threshold = Utc::now() + Duration::seconds(REFRESH_MARGIN_SECS);
                let still_valid = cached.as_ref()
                    .map(|c| c.expires_at > refresh_threshold)
                    .unwrap_or(false);
                if still_valid {
                    return cached.as_ref().map(|c| c.creds.clone());
                }
                match assume_role_with_web_identity(role_arn, token_file, region) {
                    Ok((creds, expires_at)) => {
                        *cached = Some(CachedCredentials { creds: creds.clone(), expires_at });
                        Some(creds)
                    }
                    Err(e) => {
                        log_error!("[s3/irsa] STS credential refresh failed: {e}");
                        // Return stale cached creds during a grace period rather than
                        // dropping all requests on a transient STS outage.
                        cached.as_ref().map(|c| c.creds.clone())
                    }
                }
            }
        }
    }
}

// ─── STS: AssumeRoleWithWebIdentity ──────────────────────────────────────────

/// Exchange a Kubernetes-projected OIDC token for short-lived STS credentials.
///
/// `AssumeRoleWithWebIdentity` does not require SigV4 signing — the web identity
/// token itself is the proof of identity.
fn assume_role_with_web_identity(
    role_arn:   &str,
    token_file: &str,
    region:     &str,
) -> Result<(AwsCredentials, DateTime<Utc>), String> {
    let token = std::fs::read_to_string(token_file)
        .map_err(|e| format!("reading web identity token from '{token_file}': {e}"))?;
    let token = token.trim();

    let url = format!("https://sts.{region}.amazonaws.com/");
    let body = format!(
        "Action=AssumeRoleWithWebIdentity&RoleArn={}&WebIdentityToken={}&RoleSessionName=orkester&Version=2011-06-15",
        urlenc(role_arn),
        urlenc(token),
    );
    let xml = ureq::post(&url)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .send_string(&body)
        .map_err(|e| format!("STS request failed: {e}"))?
        .into_string()
        .map_err(|e| format!("STS response read failed: {e}"))?;

    parse_sts_response(&xml)
}

fn parse_sts_response(xml: &str) -> Result<(AwsCredentials, DateTime<Utc>), String> {
    let access_key_id     = extract_tag(xml, "AccessKeyId")
        .ok_or("STS response missing AccessKeyId")?;
    let secret_access_key = extract_tag(xml, "SecretAccessKey")
        .ok_or("STS response missing SecretAccessKey")?;
    let session_token     = extract_tag(xml, "SessionToken")
        .ok_or("STS response missing SessionToken")?;
    let expiration        = extract_tag(xml, "Expiration")
        .ok_or("STS response missing Expiration")?;

    let expires_at = expiration.parse::<DateTime<Utc>>()
        .map_err(|e| format!("invalid STS Expiration value '{expiration}': {e}"))?;

    Ok((AwsCredentials {
        access_key_id,
        secret_access_key,
        session_token: Some(session_token),
    }, expires_at))
}

fn extract_tag(xml: &str, tag: &str) -> Option<String> {
    let open  = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end   = xml[start..].find(&close)?;
    Some(xml[start..start + end].to_string())
}

fn urlenc(s: &str) -> String {
    s.chars().map(|c| match c {
        'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
        _ => format!("%{:02X}", c as u32),
    }).collect()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn static_cfg() -> S3LoaderEntryConfig {
        S3LoaderEntryConfig {
            bucket:             "test-bucket".into(),
            prefix:             "".into(),
            region:             "us-east-1".into(),
            endpoint_url:       None,
            access_key_id:      Some("AKID".into()),
            secret_access_key:  Some("SECRET".into()),
            recursive:          false,
            watch:              false,
            poll_interval_secs: 30,
        }
    }

    fn unsigned_cfg() -> S3LoaderEntryConfig {
        S3LoaderEntryConfig {
            bucket:             "test-bucket".into(),
            prefix:             "".into(),
            region:             "us-east-1".into(),
            endpoint_url:       None,
            access_key_id:      None,
            secret_access_key:  None,
            recursive:          false,
            watch:              false,
            poll_interval_secs: 30,
        }
    }

    #[test]
    fn static_credentials_resolve() {
        let mut provider = CredentialsProvider::from_config(&static_cfg());
        let creds = provider.resolve().expect("static credentials should resolve");
        assert_eq!(creds.access_key_id, "AKID");
        assert_eq!(creds.secret_access_key, "SECRET");
        assert!(creds.session_token.is_none());
    }

    #[test]
    fn unsigned_resolves_to_none() {
        // Clear IRSA env vars to ensure we get Unsigned, not Irsa.
        // SAFETY: single-threaded test; no other threads reading these vars.
        unsafe {
            std::env::remove_var("AWS_WEB_IDENTITY_TOKEN_FILE");
            std::env::remove_var("AWS_ROLE_ARN");
        }
        let mut provider = CredentialsProvider::from_config(&unsigned_cfg());
        assert!(provider.resolve().is_none());
    }

    #[test]
    fn extract_tag_roundtrip() {
        let xml = "<Root><AccessKeyId>AKIA123</AccessKeyId><Other>x</Other></Root>";
        assert_eq!(extract_tag(xml, "AccessKeyId").as_deref(), Some("AKIA123"));
        assert_eq!(extract_tag(xml, "Missing"), None);
    }

    #[test]
    fn urlenc_encodes_special_chars() {
        let encoded = urlenc("arn:aws:iam::123:role/my-role");
        assert!(encoded.contains("%3A")); // ':' → %3A
        assert!(encoded.contains("my-role")); // '-' is safe
    }
}
