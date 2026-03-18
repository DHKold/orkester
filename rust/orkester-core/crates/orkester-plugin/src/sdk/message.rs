use std::marker::PhantomData;

use crate::abi;
use super::constants::{FLAG_NONE, FLAG_UTF8, MSG_TYPE_BYTES, MSG_TYPE_JSON, MSG_TYPE_STRING};
use super::error::Error;

// ─── Request ─────────────────────────────────────────────────────────────────

/// A message to be sent to a [`Component`](super::component::Component).
///
/// Build one with the typed constructors ([`Request::bytes`], [`Request::string`],
/// [`Request::json`]) or with [`Request::new`] for full control over format and
/// flags.
pub struct Request {
    pub(crate) id: u64,
    pub(crate) format: u32,
    pub(crate) flags: u32,
    pub(crate) payload: Vec<u8>,
}

impl Request {
    /// Create a request with explicit format, flags, and raw payload.
    pub fn new(id: u64, format: u32, flags: u32, payload: Vec<u8>) -> Self {
        Self { id, format, flags, payload }
    }

    /// Create a raw-bytes request (`MSG_TYPE_BYTES`).
    pub fn bytes(id: u64, payload: Vec<u8>) -> Self {
        Self::new(id, MSG_TYPE_BYTES, FLAG_NONE, payload)
    }

    /// Create a UTF-8 string request (`MSG_TYPE_STRING`).
    pub fn string(id: u64, text: impl Into<String>) -> Self {
        Self::new(id, MSG_TYPE_STRING, FLAG_UTF8, text.into().into_bytes())
    }

    /// Serialize `value` to JSON and create a JSON request (`MSG_TYPE_JSON`).
    pub fn json<T: serde::Serialize>(id: u64, value: &T) -> Result<Self, Error> {
        let payload = serde_json::to_vec(value)?;
        Ok(Self::new(id, MSG_TYPE_JSON, FLAG_UTF8, payload))
    }

    /// The correlation ID that will be echoed in the response.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// The format code (see `MSG_TYPE_*` constants).
    pub fn format(&self) -> u32 {
        self.format
    }

    /// The flags (see `FLAG_*` constants).
    pub fn flags(&self) -> u32 {
        self.flags
    }

    /// A view of the raw payload bytes.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Build the raw ABI request.
    ///
    /// The returned value borrows from `self`; do not drop `self` while the
    /// pointer inside is still in use.
    pub(crate) fn as_abi(&self) -> abi::Request {
        abi::Request {
            id: self.id,
            format: self.format,
            flags: self.flags,
            payload: self.payload.as_ptr(),
            len: self.payload.len() as u32,
        }
    }

    /// Reconstruct a [`Request`] from a raw ABI request, copying the payload.
    ///
    /// Used by the SDK trampolines inside [`alloc_component`] to pass a safe
    /// request value to [`ComponentHandler::handle`].
    ///
    /// # Safety
    /// `req.payload` must be valid for `req.len` bytes.
    ///
    /// [`alloc_component`]: super::handler::alloc_component
    /// [`ComponentHandler::handle`]: super::handler::ComponentHandler::handle
    pub(crate) fn from_abi(req: abi::Request) -> Self {
        let payload = if req.len == 0 || req.payload.is_null() {
            Vec::new()
        } else {
            // SAFETY: guaranteed by the caller (the host).
            unsafe { std::slice::from_raw_parts(req.payload, req.len as usize) }.to_vec()
        };
        Self { id: req.id, format: req.format, flags: req.flags, payload }
    }
}

// ─── ComponentResponse ────────────────────────────────────────────────────────

/// A response received from a [`Component`](super::component::Component).
///
/// The payload buffer was allocated by the plugin.  When this value is dropped
/// the buffer is released back to the plugin through its `free_response`
/// function pointer — callers must not hold on to the bytes slice longer than
/// the `ComponentResponse` itself.
pub struct ComponentResponse<'comp> {
    pub(crate) inner: abi::Response,
    /// Pointer to the component that produced this response and must free it.
    pub(crate) component: *mut abi::Component,
    /// Ties the response lifetime to the owning Component borrow.
    pub(crate) _lifetime: PhantomData<&'comp ()>,
}

impl<'comp> ComponentResponse<'comp> {
    /// Correlation ID echoed from the request.
    pub fn id(&self) -> u64 {
        self.inner.id
    }

    /// Format code of the payload (see `MSG_TYPE_*` constants).
    pub fn format(&self) -> u32 {
        self.inner.format
    }

    /// Flags associated with the response (see `FLAG_*` constants).
    pub fn flags(&self) -> u32 {
        self.inner.flags
    }

    /// A slice over the raw payload bytes.
    pub fn as_bytes(&self) -> &[u8] {
        if self.inner.len == 0 || self.inner.payload.is_null() {
            &[]
        } else {
            // SAFETY: the plugin guarantees `payload` points to `len` valid bytes
            // for the lifetime of this response.
            unsafe { std::slice::from_raw_parts(self.inner.payload, self.inner.len as usize) }
        }
    }

    /// Interpret the payload as a UTF-8 string slice.
    pub fn as_str(&self) -> Result<&str, Error> {
        std::str::from_utf8(self.as_bytes()).map_err(Error::from)
    }

    /// Deserialize the payload as JSON into `T`.
    pub fn as_json<T: serde::de::DeserializeOwned>(&self) -> Result<T, Error> {
        serde_json::from_slice(self.as_bytes()).map_err(Error::from)
    }
}

impl<'comp> Drop for ComponentResponse<'comp> {
    fn drop(&mut self) {
        // SAFETY: `component` is valid for the lifetime `'comp`, which outlives
        // this response.  We use `ptr::read` to produce a by-value copy of the
        // raw `Response` (which holds no drop glue itself) so we can hand
        // ownership to the plugin's free_response function.
        unsafe {
            let free_fn = (*self.component).free_response;
            let res = std::ptr::read(&self.inner);
            free_fn(self.component, res);
        }
    }
}
