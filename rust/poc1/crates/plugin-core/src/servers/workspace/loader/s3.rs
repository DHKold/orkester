//! S3 loader — polls an S3 prefix for YAML files on a configurable interval.
//!
//! **This is a stub implementation.** The poll loop is operational but the
//! actual S3 API calls are left as `todo!()` until an AWS SDK dependency is
//! added. The structure, config contract, and change-detection strategy are
//! all in place.
//!
//! # Config
//! ```yaml
//! loaders:
//!   - type: s3
//!     bucket: my-bucket
//!     prefix: objects/
//!     poll_interval_seconds: 60   # default: 60
//! ```
//!
//! # Change detection strategy
//! On every poll cycle the loader fetches the ETag (or LastModified) of each
//! `.yaml`/`.yml` object under `prefix`.  It compares the ETags to the
//! previous cycle and:
//! - emits `Upserted` for any new or changed object
//! - emits `Removed` for any object that has disappeared

use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use orkester_common::{log_debug, log_info, log_warn};
use tokio::sync::mpsc::UnboundedSender;

use super::{parse_yaml_documents, LoaderError, LoaderEvent, ObjectLoader};
use orkester_common::domain::ObjectEnvelope;

// ── S3Loader ──────────────────────────────────────────────────────────────────

pub struct S3Loader {
    bucket: String,
    prefix: String,
    poll_interval: Duration,
}

impl S3Loader {
    pub fn new(bucket: &str, prefix: &str, poll_interval_seconds: u64) -> Self {
        Self {
            bucket: bucket.to_string(),
            prefix: prefix.to_string(),
            poll_interval: Duration::from_secs(poll_interval_seconds),
        }
    }
}

#[async_trait]
impl ObjectLoader for S3Loader {
    async fn load_all(&self) -> Result<Vec<ObjectEnvelope>, LoaderError> {
        log_info!("Initial load from s3://{}/{}", self.bucket, self.prefix);
        fetch_all(&self.bucket, &self.prefix).await
    }

    async fn watch(&self, tx: UnboundedSender<LoaderEvent>) {
        let bucket = self.bucket.clone();
        let prefix = self.prefix.clone();
        let interval = self.poll_interval;

        tokio::spawn(async move {
            // key → (etag, objects) from the previous poll cycle.
            let mut known: HashMap<String, (String, Vec<ObjectEnvelope>)> = HashMap::new();

            log_info!(
                "Starting poll loop for s3://{}/{} (interval={:?})",
                bucket,
                prefix,
                interval
            );

            loop {
                tokio::time::sleep(interval).await;

                log_debug!("Polling s3://{}/{}", bucket, prefix);

                match list_objects(&bucket, &prefix).await {
                    Ok(listing) => {
                        // ── Upserts: new or changed keys ─────────────────────
                        for (key, etag) in &listing {
                            let changed = known
                                .get(key)
                                .map(|(old_etag, _)| old_etag != etag)
                                .unwrap_or(true);

                            if changed {
                                match fetch_object(&bucket, key).await {
                                    Ok(content) => {
                                        match parse_yaml_documents(&content, key) {
                                            Ok(new_objs) => {
                                                log_info!(
                                                    "Object changed: '{}' ({} document(s))",
                                                    key,
                                                    new_objs.len()
                                                );
                                                // Emit Removed for objects that
                                                // were in the old version but
                                                // are gone from the new one.
                                                if let Some((_, old_objs)) = known.get(key) {
                                                    for old in old_objs {
                                                        if !new_objs.iter().any(|n| {
                                                            n.kind() == old.kind()
                                                                && n.name() == old.name()
                                                        }) {
                                                            let _ = tx.send(LoaderEvent::Removed(
                                                                old.clone(),
                                                            ));
                                                        }
                                                    }
                                                }
                                                for obj in &new_objs {
                                                    let _ =
                                                        tx.send(LoaderEvent::Upserted(obj.clone()));
                                                }
                                                known.insert(key.clone(), (etag.clone(), new_objs));
                                            }
                                            Err(e) => {
                                                log_warn!("Parse error for '{}': {}", key, e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        log_warn!("Could not fetch '{}': {}", key, e);
                                    }
                                }
                            }
                        }

                        // ── Removals: keys that no longer exist ───────────────
                        let removed_keys: Vec<String> = known
                            .keys()
                            .filter(|k| !listing.contains_key(*k))
                            .cloned()
                            .collect();

                        for key in removed_keys {
                            if let Some((_, old_objs)) = known.remove(&key) {
                                log_info!(
                                    "Object removed: '{}' ({} document(s) deleted)",
                                    key,
                                    old_objs.len()
                                );
                                for obj in old_objs {
                                    let _ = tx.send(LoaderEvent::Removed(obj));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log_warn!("Listing failed for s3://{}/{}: {}", bucket, prefix, e);
                    }
                }
            }
        });
    }
}

// ── S3 stubs ──────────────────────────────────────────────────────────────────
//
// Replace these with real `aws_sdk_s3` calls once the crate is added to
// Cargo.toml.

/// Returns (key → etag) for every `.yaml`/`.yml` object under `prefix`.
async fn list_objects(
    _bucket: &str,
    _prefix: &str,
) -> Result<HashMap<String, String>, LoaderError> {
    // TODO: implement using aws_sdk_s3::Client::list_objects_v2
    Ok(HashMap::new())
}

/// Fetch the raw string content of a single S3 object.
async fn fetch_object(_bucket: &str, _key: &str) -> Result<String, LoaderError> {
    // TODO: implement using aws_sdk_s3::Client::get_object
    Ok(String::new())
}

/// Fetch all YAML objects under `prefix` as parsed envelopes.
async fn fetch_all(_bucket: &str, _prefix: &str) -> Result<Vec<ObjectEnvelope>, LoaderError> {
    // TODO: implement full scan using list_objects + fetch_object
    Ok(Vec::new())
}
