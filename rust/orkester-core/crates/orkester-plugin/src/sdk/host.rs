use std::ffi::c_void;

use crate::abi;
use super::constants::{FLAG_NONE, MSG_TYPE_BYTES, PROTOCOL_V1};

// ─── HostHandler trait ────────────────────────────────────────────────────────

/// Implemented by types that handle requests coming *from* a plugin back to the
/// host (e.g. logging, resource access, capability negotiation).
///
/// The implementation receives the decoded payload slice and must return:
/// - the response payload bytes,
/// - the format code for those bytes (see `MSG_TYPE_*` constants),
/// - the flags for the response (see `FLAG_*` constants).
///
/// Returning an empty `Vec` is valid and produces a zero-length response.
///
/// # Thread safety
/// Plugins may call back from worker threads, so the handler must be
/// `Send + Sync`.
pub trait HostHandler: Send + Sync + 'static {
    fn handle(&self, id: u64, format: u32, flags: u32, payload: &[u8]) -> (Vec<u8>, u32, u32);
}

// ─── NullHostHandler ──────────────────────────────────────────────────────────

/// A no-op [`HostHandler`] that returns an empty `MSG_TYPE_BYTES` response for
/// every request.  Use this when the plugin does not need to call back into the
/// host.
pub struct NullHostHandler;

impl HostHandler for NullHostHandler {
    fn handle(&self, id: u64, _format: u32, _flags: u32, _payload: &[u8]) -> (Vec<u8>, u32, u32) {
        let _ = id;
        (Vec::new(), MSG_TYPE_BYTES, FLAG_NONE)
    }
}

// ─── Internal state ───────────────────────────────────────────────────────────

/// Heap-allocated handler state.  The `context` field of `abi::Host` always
/// points here so the C trampolines can recover the handler regardless of
/// where the `abi::Host` struct itself lives.
struct HostInner {
    handler: Box<dyn HostHandler>,
}

// ─── OrkesterHost ─────────────────────────────────────────────────────────────

/// A host-side endpoint that can be handed to a plugin via
/// [`Plugin::load`](super::plugin::Plugin::load).
///
/// Both the `abi::Host` struct and the handler state are stored on the heap, so
/// the pointer passed to the plugin remains stable even if this struct is later
/// moved (e.g. into a `Plugin`).
///
/// # Lifetime contract
/// The plugin receives a `*mut abi::Host` and may call back through it at any
/// time.  You must ensure this `OrkesterHost` (or the [`Plugin`] that owns it)
/// lives at least as long as the root component produced by the plugin.
pub struct OrkesterHost {
    /// Heap-stable `abi::Host`; the pointer we give to the plugin.
    ffi: Box<abi::Host>,
    /// Heap-stable handler; `ffi.context` points here.
    _inner: Box<HostInner>,
}

// SAFETY: both heap allocations are exclusively owned and HostHandler: Send+Sync.
unsafe impl Send for OrkesterHost {}
unsafe impl Sync for OrkesterHost {}

impl OrkesterHost {
    /// Create a host backed by a custom handler.
    pub fn new(handler: impl HostHandler) -> Self {
        let inner = Box::new(HostInner {
            handler: Box::new(handler),
        });
        let ffi = Box::new(abi::Host {
            protocol: PROTOCOL_V1,
            // context points to the heap-stable HostInner allocation.
            context: inner.as_ref() as *const HostInner as *mut c_void,
            handle: abi_host_handle,
            free_response: abi_host_free_response,
        });
        Self { ffi, _inner: inner }
    }

    /// Create a host with the no-op [`NullHostHandler`].
    pub fn null() -> Self {
        Self::new(NullHostHandler)
    }

    /// Returns a raw pointer to the stable `abi::Host` struct.
    ///
    /// Valid until this `OrkesterHost` is dropped.
    pub(crate) fn as_ptr(&mut self) -> *mut abi::Host {
        self.ffi.as_mut() as *mut abi::Host
    }
}

// ─── C-callable trampolines ───────────────────────────────────────────────────

unsafe extern "C" fn abi_host_handle(this: *mut abi::Host, req: abi::Request) -> abi::Response {
    // SAFETY: `this` is our `abi::Host` whose `context` we set to a valid
    // `*mut HostInner`; both live at least as long as the plugin's root
    // component.
    let inner = unsafe { &*((*this).context as *const HostInner) };

    let payload_slice = if req.len == 0 || req.payload.is_null() {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(req.payload, req.len as usize) }
    };

    let (resp_bytes, format, flags) =
        inner.handler.handle(req.id, req.format, req.flags, payload_slice);

    // Transfer buffer ownership to the caller.  We allocate via Box<[u8]> so
    // `abi_host_free_response` can reclaim it with the matching Box::from_raw.
    let (ptr, len) = if resp_bytes.is_empty() {
        (std::ptr::null_mut::<u8>(), 0u32)
    } else {
        let mut boxed: Box<[u8]> = resp_bytes.into_boxed_slice();
        let ptr = boxed.as_mut_ptr();
        let len = boxed.len() as u32;
        std::mem::forget(boxed);
        (ptr, len)
    };

    abi::Response { id: req.id, format, flags, payload: ptr, len }
}

unsafe extern "C" fn abi_host_free_response(_this: *mut abi::Host, res: abi::Response) {
    if !res.payload.is_null() && res.len > 0 {
        // SAFETY: the buffer was allocated in `abi_host_handle` via
        // `Box<[u8]>::into_raw`; we reconstruct the same layout here.
        unsafe {
            drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                res.payload,
                res.len as usize,
            )));
        }
    }
}
