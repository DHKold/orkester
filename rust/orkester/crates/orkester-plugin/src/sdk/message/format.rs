/// Standard JSON-encoded payload: `{ "action": "...", "params": ... }`.
pub const JSON: &str = "std/json";

/// Standard YAML-encoded payload.
pub const YAML: &str = "std/yaml";

/// Standard MessagePack-encoded payload.
pub const MSGPACK: &str = "std/msgpack";

/// Response payload carrying a heap-allocated `*mut AbiComponent` pointer.
pub const COMPONENT: &str = "orkester/component";

/// Decode `bytes` according to `format` into a deserializable `T`.
/// Returns an error for unrecognised formats rather than guessing.
pub fn decode<T: serde::de::DeserializeOwned>(format: &str, bytes: &[u8]) -> crate::sdk::error::Result<T> {
    match format {
        JSON => Ok(serde_json::from_slice(bytes)?),
        YAML => Ok(serde_yaml::from_slice(bytes)?),
        MSGPACK => Ok(rmp_serde::from_slice(bytes)?),
        other => Err(format!("unsupported format: {other}").into()),
    }
}
