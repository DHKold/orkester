use crate::abi;
use super::constants::{ERROR_UNSUPPORTED, MSG_TYPE_JSON, PROTOCOL_V1};
use super::message::Request;

// ─── ComponentHandler trait ───────────────────────────────────────────────────

/// The only trait you implement to create a component.
///
/// Annotate your struct with whatever fields you need (state, config, …) and
/// implement this one method.  The SDK generates all ABI boilerplate for you
/// via [`alloc_component`].
///
/// # Example
/// ```ignore
/// use orkester_plugin::sdk::{ComponentHandler, Request, Response};
///
/// struct Echo;
///
/// impl ComponentHandler for Echo {
///     fn handle(&mut self, req: Request) -> Response {
///         Response::bytes(req.id(), req.payload().to_vec())
///     }
/// }
/// ```
pub trait ComponentHandler: Send + Sync + 'static {
    /// Process `req` and return a response.
    ///
    /// The response is heap-allocated by the SDK and freed for you when the
    /// host calls `free_response`.
    fn handle(&mut self, req: Request) -> Response;
}

// ─── Response builder ─────────────────────────────────────────────────────────

/// A response value returned from [`ComponentHandler::handle`].
///
/// Construct one with the typed factory methods, or build it directly for full
/// format/flags control (e.g. the echo component mirrors the request format).
pub struct Response {
    pub format: u32,
    pub flags:  u32,
    pub payload: Vec<u8>,
}

impl Response {
    /// Raw bytes (`MSG_TYPE_BYTES`).
    pub fn bytes(payload: Vec<u8>) -> Self {
        use super::constants::{FLAG_NONE, MSG_TYPE_BYTES};
        Self { format: MSG_TYPE_BYTES, flags: FLAG_NONE, payload }
    }

    /// UTF-8 string (`MSG_TYPE_STRING`).
    pub fn string(text: impl Into<String>) -> Self {
        use super::constants::{FLAG_UTF8, MSG_TYPE_STRING};
        Self { format: MSG_TYPE_STRING, flags: FLAG_UTF8, payload: text.into().into_bytes() }
    }

    /// Serialise `value` to JSON.  Panics if serialization fails (which only
    /// happens for types with non-string map keys in unusual circumstances).
    pub fn json<T: serde::Serialize>(value: &T) -> Self {
        use super::constants::{FLAG_UTF8, MSG_TYPE_JSON};
        let payload = serde_json::to_vec(value)
            .expect("Response::json: serialization failed");
        Self { format: MSG_TYPE_JSON, flags: FLAG_UTF8, payload }
    }

    /// Encode a `*mut abi::Component` pointer as native-endian bytes
    /// (`MSG_TYPE_POINTER`).  Used by factory components.
    pub fn pointer(ptr: *mut abi::Component) -> Self {
        use super::constants::{FLAG_NONE, MSG_TYPE_POINTER};
        let bytes = (ptr as usize).to_ne_bytes().to_vec();
        Self { format: MSG_TYPE_POINTER, flags: FLAG_NONE, payload: bytes }
    }

    /// Return a structured error.
    /// ```json
    /// { "error": <code>, "message": "<msg>" }
    /// ```
    pub fn error(code: u32, message: impl Into<String>) -> Self {
        use super::constants::{FLAG_UTF8, MSG_TYPE_JSON};
        #[derive(serde::Serialize)]
        struct E { error: u32, message: String }
        let payload = serde_json::to_vec(&E { error: code, message: message.into() })
            .unwrap_or_default();
        Self { format: MSG_TYPE_JSON, flags: FLAG_UTF8, payload }
    }
}

// ─── JSON request helper ──────────────────────────────────────────────────────

/// Verify that `req` carries a JSON payload and deserialise it into `T`.
///
/// Returns `Err(Response::error(...))` when:
/// - the request format is not `MSG_TYPE_JSON` (`ERROR_UNSUPPORTED`), or
/// - the payload cannot be deserialised into `T` (`ERROR_INVALID_REQUEST`).
///
/// # Usage
/// ```ignore
/// fn handle(&mut self, req: Request) -> Response {
///     let parsed: MyReq = match require_json(&req) {
///         Ok(v)  => v,
///         Err(e) => return e,
///     };
///     // ... rest of handler
/// }
/// ```
pub fn require_json<T: serde::de::DeserializeOwned>(req: &Request) -> Result<T, Response> {
    use super::constants::ERROR_INVALID_REQUEST;
    if req.format() != MSG_TYPE_JSON {
        return Err(Response::error(
            ERROR_UNSUPPORTED,
            format!(
                "expected JSON request (format {}), got format {}",
                MSG_TYPE_JSON,
                req.format()
            ),
        ));
    }
    serde_json::from_slice(req.payload()).map_err(|e| {
        Response::error(ERROR_INVALID_REQUEST, e.to_string())
    })
}

// ─── Component allocator ──────────────────────────────────────────────────────

/// Allocate an `abi::Component` whose `handle` / `free_response` / `free`
/// trampolines are generated by the SDK.
///
/// Call this wherever you would previously have written `Box::into_raw`,
/// `#[repr(C)]`, and hand-written trampoline functions.
///
/// # Arguments
/// - `id`        — unique component ID (e.g. from an [`AtomicU32`] counter).
/// - `kind`      — component kind code (one of your `KIND_*` constants).
/// - `parent_id` — ID of the parent component (`0` for root).
/// - `handler`   — your [`ComponentHandler`] implementation.
///
/// # Ownership
/// Returns a raw `*mut abi::Component`.  Ownership is transferred to the
/// caller (typically the host layer or a pointer response).  The component
/// must eventually be freed: the SDK trampoline calls `drop` on the boxed
/// handler when `free` is invoked.
pub fn alloc_component<H: ComponentHandler>(
    id: u32,
    kind: u32,
    parent_id: u32,
    handler: H,
) -> *mut abi::Component {
    // We store the handler in a second allocation reachable via `context`.
    // The outer `abi::Component` is what we hand to the host; its `context`
    // pointer points to the handler box.
    let handler_ptr = Box::into_raw(Box::new(handler)) as *mut ();

    let comp = Box::new(abi::Component {
        protocol: PROTOCOL_V1,
        id,
        kind,
        parent: parent_id,
        context: handler_ptr as *mut std::ffi::c_void,
        handle: sdk_handle::<H>,
        free_response: sdk_free_response,
        free: sdk_free::<H>,
    });
    Box::into_raw(comp)
}

// ─── SDK-generated trampolines ────────────────────────────────────────────────

unsafe extern "C" fn sdk_handle<H: ComponentHandler>(
    this: *mut abi::Component,
    req: abi::Request,
) -> abi::Response {
    // SAFETY: `this.context` was set in `alloc_component` to `*mut H`.
    let handler = unsafe { &mut *((*this).context as *mut H) };
    let req_id = req.id;
    let request = Request::from_abi(req);
    let response = handler.handle(request);
    sdk_alloc_response(req_id, response)
}

fn sdk_alloc_response(id: u64, r: Response) -> abi::Response {
    let (ptr, len) = if r.payload.is_empty() {
        (std::ptr::null_mut::<u8>(), 0u32)
    } else {
        let mut boxed: Box<[u8]> = r.payload.into_boxed_slice();
        let ptr = boxed.as_mut_ptr();
        let len = boxed.len() as u32;
        std::mem::forget(boxed);
        (ptr, len)
    };
    abi::Response { id, format: r.format, flags: r.flags, payload: ptr, len }
}

unsafe extern "C" fn sdk_free_response(
    _this: *mut abi::Component,
    res: abi::Response,
) {
    if !res.payload.is_null() && res.len > 0 {
        unsafe {
            drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                res.payload,
                res.len as usize,
            )));
        }
    }
}

unsafe extern "C" fn sdk_free<H: ComponentHandler>(this: *mut abi::Component) {
    unsafe {
        // Free the handler first.
        drop(Box::from_raw((*this).context as *mut H));
        // Free the component header.
        drop(Box::from_raw(this));
    }
}
