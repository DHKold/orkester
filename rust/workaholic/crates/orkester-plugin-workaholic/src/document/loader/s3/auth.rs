//! AWS Signature V4 helpers for S3 requests.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;
const EMPTY_HASH: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

pub(super) fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn hmac256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn derive_signing_key(secret: &str, date: &str, region: &str) -> Vec<u8> {
    let k1 = hmac256(format!("AWS4{secret}").as_bytes(), date.as_bytes());
    let k2 = hmac256(&k1, region.as_bytes());
    let k3 = hmac256(&k2, b"s3");
    hmac256(&k3, b"aws4_request")
}

fn canonical_request(method: &str, path: &str, query: &str, host: &str, datetime: &str) -> String {
    let canon_headers = format!("host:{host}\nx-amz-date:{datetime}\n");
    format!("{method}\n{path}\n{query}\n{canon_headers}\nhost;x-amz-date\n{EMPTY_HASH}")
}

fn string_to_sign(datetime: &str, date: &str, region: &str, canon_req: &str) -> String {
    let scope = format!("{date}/{region}/s3/aws4_request");
    let req_hash = sha256_hex(canon_req.as_bytes());
    format!("AWS4-HMAC-SHA256\n{datetime}\n{scope}\n{req_hash}")
}

/// Compute the `Authorization` header value for an S3 GET request.
pub fn authorization_header(
    method:     &str,
    path:       &str,
    query:      &str,
    host:       &str,
    datetime:   &str,
    date:       &str,
    region:     &str,
    access_key: &str,
    secret_key: &str,
) -> String {
    let signing_key = derive_signing_key(secret_key, date, region);
    let canon_req   = canonical_request(method, path, query, host, datetime);
    let sts         = string_to_sign(datetime, date, region, &canon_req);
    let sig         = hex::encode(hmac256(&signing_key, sts.as_bytes()));
    let scope       = format!("{date}/{region}/s3/aws4_request");
    format!("AWS4-HMAC-SHA256 Credential={access_key}/{scope},SignedHeaders=host;x-amz-date,Signature={sig}")
}
