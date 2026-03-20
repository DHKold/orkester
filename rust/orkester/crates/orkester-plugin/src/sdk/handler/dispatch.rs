use std::{collections::HashMap, slice};
use crate::abi::{AbiComponent, AbiRequest, AbiResponse, ORKESTER_ABI_VERSION};
use crate::sdk::message::format;

// ── Handler types ─────────────────────────────────────────────────────────────

/// A type-erased handler: receives `(component, format, payload)` and returns
/// serialized response bytes, or an error message.
pub(super) type Handler<C> =
    Box<dyn Fn(&mut C, &str, &[u8]) -> Result<Vec<u8>, String> + Send + Sync>;

/// A type-erased factory: produces a boxed `AbiComponent`, or an error.
pub(super) type Factory<C> =
    Box<dyn Fn(&mut C, &str, &[u8]) -> Result<AbiComponent, String> + Send + Sync>;

// ── DispatchTable ─────────────────────────────────────────────────────────────

pub(super) struct DispatchTable<C: Send + 'static> {
    pub(super) component: C,
    pub(super) handlers: HashMap<String, Handler<C>>,
    pub(super) factories: HashMap<String, Factory<C>>,
}

impl<C: Send + 'static> DispatchTable<C> {
    pub fn dispatch(&mut self, req: &AbiRequest) -> OwnedResponse {
        let fmt = unsafe { read_str(req.format, req.format_len) };
        if fmt != format::JSON {
            return OwnedResponse::error(req.id, &format!("unsupported envelope format: {fmt}"));
        }
        let payload = unsafe { read_bytes(req.payload, req.payload_len) };
        let envelope: serde_json::Value = match serde_json::from_slice(payload) {
            Ok(v) => v,
            Err(e) => return OwnedResponse::error(req.id, &e.to_string()),
        };
        let action = match envelope["action"].as_str() {
            Some(a) => a,
            None => return OwnedResponse::error(req.id, "missing 'action' field in envelope"),
        };
        let params_bytes = match serde_json::to_vec(&envelope["params"]) {
            Ok(b) => b,
            Err(e) => return OwnedResponse::error(req.id, &e.to_string()),
        };

        if action == "orkester/CreateComponent" {
            return self.dispatch_factory(&envelope["params"], req.id);
        }

        match self.handlers.get(action) {
            Some(h) => match h(&mut self.component, format::JSON, &params_bytes) {
                Ok(bytes) => OwnedResponse::payload(req.id, format::JSON, bytes),
                Err(e) => OwnedResponse::error(req.id, &e),
            },
            None => OwnedResponse::error(req.id, &format!("no handler for action '{action}'")),
        }
    }

    fn dispatch_factory(&mut self, params: &serde_json::Value, id: u64) -> OwnedResponse {
        let kind = match params["kind"].as_str() {
            Some(k) => k,
            None => return OwnedResponse::error(id, "missing 'kind' field in CreateComponent params"),
        };
        let config_bytes = match serde_json::to_vec(&params["config"]) {
            Ok(b) => b,
            Err(e) => return OwnedResponse::error(id, &e.to_string()),
        };
        match self.factories.get(kind) {
            Some(f) => match f(&mut self.component, format::JSON, &config_bytes) {
                Ok(abi) => {
                    let ptr = Box::into_raw(Box::new(abi));
                    OwnedResponse::component(id, ptr)
                }
                Err(e) => OwnedResponse::error(id, &e),
            },
            None => OwnedResponse::error(id, &format!("no factory for kind '{kind}'")),
        }
    }
}

// ── OwnedResponse ─────────────────────────────────────────────────────────────

/// Heap-allocated response payload; converted into a raw [`AbiResponse`] for
/// the ABI boundary.  Ownership is transferred to the caller (host) who must
/// call `free_response` to release it.
pub(super) struct OwnedResponse {
    id: u64,
    format: &'static str,
    payload: Vec<u8>,
}

impl OwnedResponse {
    pub fn payload(id: u64, format: &'static str, bytes: Vec<u8>) -> Self {
        Self { id, format, payload: bytes }
    }

    pub fn error(id: u64, msg: &str) -> Self {
        let bytes = serde_json::to_vec(&serde_json::json!({ "error": msg }))
            .unwrap_or_default();
        Self { id, format: format::JSON, payload: bytes }
    }

    pub fn component(id: u64, ptr: *mut AbiComponent) -> Self {
        Self {
            id,
            format: format::COMPONENT,
            payload: (ptr as usize).to_le_bytes().to_vec(),
        }
    }

    /// Convert into a raw [`AbiResponse`].  The payload bytes are heap-allocated
    /// via `Box<[u8]>` and must be freed by `dispatch_free_response`.
    pub fn into_abi(self) -> AbiResponse {
        let mut boxed = self.payload.into_boxed_slice();
        let ptr = boxed.as_mut_ptr();
        let len = boxed.len() as u32;
        std::mem::forget(boxed);
        AbiResponse {
            id: self.id,
            format: self.format.as_ptr(),
            format_len: self.format.len() as u32,
            payload: ptr,
            payload_len: len,
        }
    }
}

// ── ABI vtable implementations ────────────────────────────────────────────────

pub unsafe extern "C" fn dispatch_handle<C: Send + 'static>(
    this: *mut AbiComponent,
    req: AbiRequest,
) -> AbiResponse {
    unsafe {
        let table = &mut *((*this).context as *mut DispatchTable<C>);
        table.dispatch(&req).into_abi()
    }
}

pub unsafe extern "C" fn dispatch_free_response(_: *mut AbiComponent, res: AbiResponse) {
    unsafe {
        if !res.payload.is_null() && res.payload_len > 0 {
            drop(Box::from_raw(slice::from_raw_parts_mut(
                res.payload,
                res.payload_len as usize,
            )));
        }
    }
}

pub unsafe extern "C" fn dispatch_free<C: Send + 'static>(this: *mut AbiComponent) {
    unsafe {
        drop(Box::from_raw((*this).context as *mut DispatchTable<C>));
        drop(Box::from_raw(this));
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

unsafe fn read_str(ptr: *const u8, len: u32) -> &'static str {
    if ptr.is_null() || len == 0 {
        return "";
    }
    let bytes = unsafe { slice::from_raw_parts(ptr, len as usize) };
    std::str::from_utf8(bytes).unwrap_or("")
}

unsafe fn read_bytes(ptr: *const u8, len: u32) -> &'static [u8] {
    if ptr.is_null() || len == 0 {
        return &[];
    }
    unsafe { slice::from_raw_parts(ptr, len as usize) }
}

// ── build helper ─────────────────────────────────────────────────────────────

pub(super) fn build_component<C: Send + 'static>(table: DispatchTable<C>) -> AbiComponent {
    let ctx = Box::into_raw(Box::new(table)) as *mut std::ffi::c_void;
    AbiComponent {
        protocol: ORKESTER_ABI_VERSION,
        context: ctx,
        handle: dispatch_handle::<C>,
        free_response: dispatch_free_response,
        free: dispatch_free::<C>,
    }
}
