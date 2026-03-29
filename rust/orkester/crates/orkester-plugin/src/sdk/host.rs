use std::ffi::OsStr;
use crate::abi::{AbiHost, AbiRequest, AbiResponse, FnRootComponentBuilder, ORKESTER_ABI_VERSION};
use crate::sdk::{
    error::Result,
    message::{codec, format, Serializer},
};

// ── HostRef ───────────────────────────────────────────────────────────────────

/// A lightweight, `Copy`-able, `Send + Sync` handle to the host ABI pointer.
///
/// Unlike [`Host`], `HostRef` does **not** own the `AbiHost` allocation; it
/// can be stored, cloned, and sent across threads freely.  Use it wherever a
/// component needs to retain a handle to the host for fire-and-forget events
/// without taking ownership (e.g. storing in a background watcher thread).
///
/// # Safety
/// The host outlives all loaded plugins, so a `HostRef` derived from the
/// pointer passed at plugin entry is always valid for the process lifetime.
#[derive(Clone, Copy)]
pub struct HostRef(*mut AbiHost);

// SAFETY: The host's ABI handle function is callable from any thread.
unsafe impl Send for HostRef {}
unsafe impl Sync for HostRef {}

impl HostRef {
    /// Wrap a raw `*mut AbiHost` pointer.
    ///
    /// # Safety
    /// `ptr` must remain valid for the lifetime of all `HostRef` values
    /// derived from it (guaranteed by the plugin contract).
    pub fn new(ptr: *mut AbiHost) -> Self {
        Self(ptr)
    }

    /// Fire a one-way event to the host (no reply expected).
    ///
    /// Serialises `value` as JSON with format `"std/json+fire"`.  The
    /// pipeline worker routes the envelope but never sends a response back.
    /// Use this for document change events and other one-way notifications.
    pub fn fire<T: serde::Serialize>(&self, value: &T) {
        static FMT: &str = "std/json+fire";
        let payload = serde_json::to_vec(value).unwrap_or_default();
        let req = AbiRequest {
            id:          0,
            format:      FMT.as_ptr(),
            format_len:  FMT.len() as u32,
            payload:     payload.as_ptr(),
            payload_len: payload.len() as u32,
        };
        unsafe {
            let res = ((*self.0).handle)(self.0, req);
            ((*self.0).free_response)(self.0, res);
        }
    }

    /// Return the raw `*mut AbiHost` pointer.
    pub fn as_ptr(&self) -> *mut AbiHost {
        self.0
    }
}

// ── Host ──────────────────────────────────────────────────────────────────────

/// SDK handle to the orkester host.
///
/// **Plugin side** — wrap the pointer received at plugin entry:
/// ```ignore
/// let host = Host::from_abi(raw_host_ptr);
/// ```
///
/// **Host side** — create a standalone instance used to load plugins:
/// ```ignore
/// let mut host = Host::new();
/// let plugin = host.load_plugin("plugin.so")?;
/// ```
pub struct Host {
    ptr: *mut AbiHost,
    /// Present when this instance _owns_ the AbiHost allocation.
    _owned: Option<Box<AbiHost>>,
    /// Keeps any additional heap allocation (e.g. a callback closure) alive for
    /// as long as this Host is alive.  The AbiHost's `context` pointer may point
    /// into this allocation.
    _extra: Option<Box<dyn std::any::Any + Send + Sync>>,
}

impl Host {
    // ── Constructors ──────────────────────────────────────────────────────

    /// Wrap a raw `*mut AbiHost` received from the runtime.
    ///
    /// # Safety
    /// `ptr` must remain valid for the lifetime of this `Host`.
    pub unsafe fn from_abi(ptr: *mut AbiHost) -> Self {
        Self { ptr, _owned: None, _extra: None }
    }

    /// Create an owned noop host instance (host side).
    ///
    /// All calls made by plugin-side components to `host.handle()` will receive
    /// an empty response.  Use [`with_callback`](Self::with_callback) to wire
    /// in real routing logic.
    pub fn new() -> Self {
        let mut host = Box::new(AbiHost {
            protocol: ORKESTER_ABI_VERSION,
            context: std::ptr::null_mut(),
            handle: noop_host_handle,
            free_response: noop_host_free_response,
        });
        let ptr = host.as_mut() as *mut AbiHost;
        Self { ptr, _owned: Some(host), _extra: None }
    }

    /// Create an owned host that routes inbound component calls through
    /// `callback`.
    ///
    /// The closure receives the raw [`AbiRequest`] and must return a heap-
    /// allocated [`AbiResponse`] whose `payload` will be freed by
    /// `callback_host_free_response`.  The simplest way to build such a
    /// response is:
    /// ```ignore
    /// let bytes = serde_json::to_vec(&result_value).unwrap();
    /// let fmt   = "std/json";
    /// AbiResponse {
    ///     id:          req.id,
    ///     format:      fmt.as_ptr(),
    ///     format_len:  fmt.len() as u32,
    ///     payload:     Box::into_raw(bytes.into_boxed_slice()) as *mut u8,
    ///     payload_len: bytes.len() as u32,
    /// }
    /// ```
    pub fn with_callback<F>(callback: F) -> Self
    where
        F: Fn(AbiRequest) -> AbiResponse + Send + Sync + 'static,
    {
        let ctx = Box::new(CallbackContext { cb: Box::new(callback) });
        // Keep a stable pointer into the heap allocation.  The Box will not
        // move because it is stored in `_extra` below and never re-boxed.
        let ctx_ptr = ctx.as_ref() as *const CallbackContext as *mut std::ffi::c_void;

        let mut abi = Box::new(AbiHost {
            protocol:      ORKESTER_ABI_VERSION,
            context:       ctx_ptr,
            handle:        callback_host_handle,
            free_response: callback_host_free_response,
        });
        let ptr = abi.as_mut() as *mut AbiHost;

        Self {
            ptr,
            _owned: Some(abi),
            _extra: Some(ctx as Box<dyn std::any::Any + Send + Sync>),
        }
    }

    /// Return the raw `*mut AbiHost` pointer.
    ///
    /// Pass this to child components that need to call back to the host.
    ///
    /// # Safety
    /// The pointer is valid only while this `Host` is alive.
    pub fn raw_ptr(&self) -> *mut AbiHost { self.ptr }
    /// Internally it serializes the request, calls the ABI handle function,
    /// copies the response payload into `R`, and frees the original buffer.
    pub fn handle<T, R>(&mut self, value: &T) -> Result<R>
    where
        T: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let req = Serializer::json(value);
        let res = unsafe { ((*self.ptr).handle)(self.ptr, req.as_abi()) };
        let result = codec::decode_response::<R>(&res);
        unsafe { ((*self.ptr).free_response)(self.ptr, res) };
        result
    }

    /// Send a fire-and-forget message to the host.
    ///
    /// Serialises `value` as JSON with format `"std/json+fire"` and calls the
    /// ABI handle.  The response (an empty ack) is freed immediately so the
    /// host pipeline can return as soon as it has enqueued the request.
    ///
    /// Use this for one-way events that do not require a reply — e.g. document
    /// change notifications from a loader component.
    pub fn fire<T: serde::Serialize>(&mut self, value: &T) {
        static FMT: &str = "std/json+fire";
        // Keep `payload` alive across the unsafe ABI call — the raw pointer is
        // only valid while the Vec is on the stack.
        let payload = serde_json::to_vec(value).unwrap_or_default();
        let req = crate::abi::AbiRequest {
            id:          0,
            format:      FMT.as_ptr(),
            format_len:  FMT.len() as u32,
            payload:     payload.as_ptr(),
            payload_len: payload.len() as u32,
        };
        let res = unsafe { ((*self.ptr).handle)(self.ptr, req) };
        unsafe { ((*self.ptr).free_response)(self.ptr, res) };
    }

    // ── Plugin loading ─────────────────────────────────────────────────────

    /// Dynamically load a plugin from a shared library.
    pub fn load_plugin(&mut self, path: impl AsRef<OsStr>) -> Result<LoadedPlugin> {
        let lib = unsafe { libloading::Library::new(path.as_ref())? };
        let entry: libloading::Symbol<FnRootComponentBuilder> = unsafe { lib.get(b"orkester_plugin_entry\0")? };
        let component = unsafe { entry(self.ptr) };
        if component.is_null() {
            return Err("plugin entry returned a null component".into());
        }
        let got = unsafe { (*component).protocol };
        if got != ORKESTER_ABI_VERSION {
            unsafe { ((*component).free)(component) };
            return Err(format!(
                "ABI version mismatch: host={ORKESTER_ABI_VERSION}, plugin={got}"
            )
            .into());
        }
        Ok(LoadedPlugin { component, _lib: lib })
    }
}

impl Default for Host {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: Host wraps a raw pointer, but we never alias it across threads.
unsafe impl Send for Host {}

// ── LoadedPlugin ──────────────────────────────────────────────────────────────

/// A plugin whose root component is live and ready for calls.
pub struct LoadedPlugin {
    pub(crate) component: *mut crate::abi::AbiComponent,
    _lib: libloading::Library,
}

impl LoadedPlugin {
    /// Return the raw root component pointer.
    ///
    /// # Safety
    /// The pointer is valid as long as this `LoadedPlugin` is alive.
    pub fn root_ptr(&self) -> *mut crate::abi::AbiComponent {
        self.component
    }

    /// Return a [`ComponentHandle`] for the root component.
    ///
    /// The handle borrows this plugin — it must not outlive it.
    pub fn get_root_component(&mut self) -> ComponentHandle<'_> {
        ComponentHandle { ptr: self.component, _marker: std::marker::PhantomData }
    }
}

impl Drop for LoadedPlugin {
    fn drop(&mut self) {
        unsafe { ((*self.component).free)(self.component) };
    }
}

unsafe impl Send for LoadedPlugin {}

// ── ComponentHandle ───────────────────────────────────────────────────────────

/// Safe, borrowing handle to a live `*mut AbiComponent`.
///
/// Automatically handles request serialization, response extraction, and
/// `free_response` — callers never touch the ABI directly.
pub struct ComponentHandle<'a> {
    pub(crate) ptr: *mut crate::abi::AbiComponent,
    _marker: std::marker::PhantomData<&'a mut ()>,
}

impl<'a> ComponentHandle<'a> {
    /// Send a typed request and decode a typed response.
    pub fn call<T, R>(&mut self, value: &T) -> Result<R>
    where
        T: serde::Serialize,
        R: serde::de::DeserializeOwned,
    {
        let req = Serializer::json(value);
        let res = unsafe { ((*self.ptr).handle)(self.ptr, req.as_abi()) };
        let result = codec::decode_response::<R>(&res);
        unsafe { ((*self.ptr).free_response)(self.ptr, res) };
        result
    }

    /// Send a request and receive a sub-component pointer.
    pub fn call_factory<T>(&mut self, value: &T) -> Result<*mut crate::abi::AbiComponent>
    where
        T: serde::Serialize,
    {
        let req = Serializer::json(value);
        let res = unsafe { ((*self.ptr).handle)(self.ptr, req.as_abi()) };
        let fmt = unsafe {
            let bytes = std::slice::from_raw_parts(res.format, res.format_len as usize);
            std::str::from_utf8(bytes).unwrap_or(format::JSON)
        };
        if fmt != format::COMPONENT {
            unsafe { ((*self.ptr).free_response)(self.ptr, res) };
            return Err(format!("expected format '{}', got '{fmt}'", format::COMPONENT).into());
        }
        codec::Deserializer::component(self.ptr, res)
    }
}

// ── No-op AbiHost vtable (used by Host::new) ─────────────────────────────────

unsafe extern "C" fn noop_host_handle(_: *mut AbiHost, req: AbiRequest) -> AbiResponse {
    AbiResponse {
        id:          req.id,
        format:      std::ptr::null(),
        format_len:  0,
        payload:     std::ptr::null_mut(),
        payload_len: 0,
    }
}

unsafe extern "C" fn noop_host_free_response(_: *mut AbiHost, _: AbiResponse) {}

// ── Callback AbiHost vtable (used by Host::with_callback) ────────────────────

type HostCallbackFn = Box<dyn Fn(AbiRequest) -> AbiResponse + Send + Sync + 'static>;

struct CallbackContext {
    cb: HostCallbackFn,
}

unsafe extern "C" fn callback_host_handle(
    this: *mut AbiHost,
    req:  AbiRequest,
) -> AbiResponse {
    unsafe {
        let ctx = &*((*this).context as *const CallbackContext);
        (ctx.cb)(req)
    }
}

/// Free a heap-allocated response produced by a `with_callback` host.
///
/// This function mirrors `dispatch_free_response` on the component side:
/// the payload slice was created with `Box::into_raw(bytes.into_boxed_slice())`.
unsafe extern "C" fn callback_host_free_response(_: *mut AbiHost, res: AbiResponse) {
    unsafe {
        if !res.payload.is_null() && res.payload_len > 0 {
            drop(Box::from_raw(std::slice::from_raw_parts_mut(
                res.payload,
                res.payload_len as usize,
            )));
        }
    }
}
