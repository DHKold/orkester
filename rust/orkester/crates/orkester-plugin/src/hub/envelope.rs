use std::sync::Arc;

// ── Envelope ──────────────────────────────────────────────────────────────────

/// An in-flight message inside the hub.
///
/// Cheap to `Clone` — all variable-length fields use shared ownership, so
/// fan-out to N dispatchers copies only the `Arc` reference counters, not the
/// actual bytes.
#[derive(Clone, Debug)]
pub struct Envelope {
    /// Hub-local unique identifier (monotonically increasing).
    pub id: u64,
    /// Optional originator identity — reserved for future authorization.
    pub owner: Option<Arc<str>>,
    /// Logical message kind; the primary routing key (e.g. `"log/Entry"`).
    pub kind: Arc<str>,
    /// Serialization format of `payload` (e.g. `"std/json"`).
    /// Visible to the router for filter evaluation; payload is never decoded
    /// by the hub core.
    pub format: Arc<str>,
    /// Opaque payload bytes.  The hub never decodes or inspects these.
    pub payload: Arc<[u8]>,
}

impl Envelope {
    pub fn new(
        id: u64,
        owner: Option<impl Into<Arc<str>>>,
        kind: impl Into<Arc<str>>,
        format: impl Into<Arc<str>>,
        payload: impl Into<Arc<[u8]>>,
    ) -> Self {
        Self {
            id,
            owner: owner.map(Into::into),
            kind: kind.into(),
            format: format.into(),
            payload: payload.into(),
        }
    }

    /// Convenience constructor: serialize `payload` as JSON and tag with `"std/json"`.
    pub fn from_json(
        id: u64,
        owner: Option<&str>,
        kind: &str,
        payload: serde_json::Value,
    ) -> Self {
        let bytes: Arc<[u8]> =
            serde_json::to_vec(&payload).unwrap_or_default().into();
        Self::new(id, owner, kind, "std/json", bytes)
    }
}
