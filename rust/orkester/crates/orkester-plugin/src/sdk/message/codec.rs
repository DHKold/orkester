use std::slice;
use crate::abi::{AbiComponent, AbiRequest, AbiResponse};
use crate::sdk::error::Result;
use super::format;

// ── OwnedRequest ─────────────────────────────────────────────────────────────

/// An [`AbiRequest`] together with its backing heap storage.
///
/// Must be kept alive for the duration of any ABI call that uses [`as_abi`].
pub struct OwnedRequest {
    format: &'static [u8],
    payload: Vec<u8>,
    id: u64,
}

static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl OwnedRequest {
    fn new(format: &'static str, payload: Vec<u8>) -> Self {
        let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self { format: format.as_bytes(), payload, id }
    }

    pub fn as_abi(&self) -> AbiRequest {
        AbiRequest {
            id: self.id,
            format: self.format.as_ptr(),
            format_len: self.format.len() as u32,
            payload: self.payload.as_ptr(),
            payload_len: self.payload.len() as u32,
        }
    }
}

// ── Serializer ────────────────────────────────────────────────────────────────

/// Creates [`OwnedRequest`] values from typed Rust objects.
pub struct Serializer;

impl Serializer {
    /// Serialize `value` as JSON.
    pub fn json<T: serde::Serialize>(value: &T) -> OwnedRequest {
        let payload = serde_json::to_vec(value).expect("JSON serialization cannot fail");
        OwnedRequest::new(format::JSON, payload)
    }
}

// ── Deserializer ──────────────────────────────────────────────────────────────

/// Extracts typed Rust values from raw [`AbiResponse`] payloads.
///
/// Every method checks the response `format`, extracts (copies) the data, then
/// calls `free_response` on the owning component so the plugin can release the
/// buffer immediately — caller never needs to call free manually.
pub struct Deserializer;

impl Deserializer {
    /// Decode a typed `T` from a component response.
    pub fn json<T: serde::de::DeserializeOwned>(
        component: *mut AbiComponent,
        res: AbiResponse,
    ) -> Result<T> {
        let value = decode_response::<T>(&res)?;
        free_response(component, res);
        Ok(value)
    }

    /// Decode a JSON [`serde_json::Value`] from a component response.
    pub fn value(component: *mut AbiComponent, res: AbiResponse) -> Result<serde_json::Value> {
        Self::json(component, res)
    }

    /// Decode a `String` from a component response.
    pub fn string(component: *mut AbiComponent, res: AbiResponse) -> Result<String> {
        Self::json(component, res)
    }

    /// Extract a sub-component pointer from a response with format `"orkester/component"`.
    ///
    /// Ownership of the returned `*mut AbiComponent` is transferred to the caller;
    /// call its `free` function when done.
    pub fn component(
        component: *mut AbiComponent,
        res: AbiResponse,
    ) -> Result<*mut AbiComponent> {
        let fmt = unsafe { read_format(&res) }.to_owned();
        if fmt != format::COMPONENT {
            free_response(component, res);
            return Err(format!("expected format '{}', got '{fmt}'", format::COMPONENT).into());
        }
        let bytes = unsafe { slice::from_raw_parts(res.payload, res.payload_len as usize) };
        if bytes.len() != std::mem::size_of::<usize>() {
            free_response(component, res);
            return Err("component response payload has wrong size for a pointer".into());
        }
        let mut arr = [0u8; std::mem::size_of::<usize>()];
        arr.copy_from_slice(bytes);
        let ptr = usize::from_le_bytes(arr) as *mut AbiComponent;
        free_response(component, res);
        Ok(ptr)
    }
}

// ── Shared helpers (pub(crate) for host.rs) ───────────────────────────────────

/// Read and copy the response payload into `T`, checking the format first.
pub(crate) fn decode_response<T: serde::de::DeserializeOwned>(res: &AbiResponse) -> Result<T> {
    let fmt = unsafe { read_format(res) };
    let payload = unsafe { read_payload(res) };
    format::decode(fmt, payload)
}

/// Call `free_response` on the component — consumes the response.
#[inline]
pub(crate) fn free_response(component: *mut AbiComponent, res: AbiResponse) {
    unsafe { ((*component).free_response)(component, res) };
}

// ── Private raw-pointer helpers ───────────────────────────────────────────────

unsafe fn read_format(res: &AbiResponse) -> &str {
    if res.format.is_null() || res.format_len == 0 {
        return format::JSON;
    }
    let bytes = unsafe { slice::from_raw_parts(res.format, res.format_len as usize) };
    std::str::from_utf8(bytes).unwrap_or(format::JSON)
}

unsafe fn read_payload(res: &AbiResponse) -> &[u8] {
    if res.payload.is_null() || res.payload_len == 0 {
        return &[];
    }
    unsafe { slice::from_raw_parts(res.payload, res.payload_len as usize) }
}
