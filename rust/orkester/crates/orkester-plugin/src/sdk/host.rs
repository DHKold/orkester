use std::ffi::OsStr;
use crate::abi::{AbiHost, AbiRequest, AbiResponse, FnRootComponentBuilder, ORKESTER_ABI_VERSION};
use crate::sdk::{
    error::Result,
    message::{codec, format, Serializer},
};

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
}

impl Host {
    // ── Constructors ──────────────────────────────────────────────────────

    /// Wrap a raw `*mut AbiHost` received from the runtime.
    ///
    /// # Safety
    /// `ptr` must remain valid for the lifetime of this `Host`.
    pub unsafe fn from_abi(ptr: *mut AbiHost) -> Self {
        Self { ptr, _owned: None }
    }

    /// Create an owned host instance (host side).
    pub fn new() -> Self {
        let mut host = Box::new(AbiHost {
            protocol: ORKESTER_ABI_VERSION,
            context: std::ptr::null_mut(),
            handle: noop_host_handle,
            free_response: noop_host_free_response,
        });
        let ptr = host.as_mut() as *mut AbiHost;
        Self { ptr, _owned: Some(host) }
    }

    // ── Plugin-side callback ───────────────────────────────────────────────

    /// Send a request to the host and receive a typed response.
    ///
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

    // ── Plugin loading ─────────────────────────────────────────────────────

    /// Dynamically load a plugin from a shared library.
    pub fn load_plugin(&mut self, path: impl AsRef<OsStr>) -> Result<LoadedPlugin> {
        let lib = unsafe { libloading::Library::new(path.as_ref())? };
        let entry: libloading::Symbol<FnRootComponentBuilder> =
            unsafe { lib.get(b"orkester_plugin_entry\0")? };
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
    component: *mut crate::abi::AbiComponent,
    _lib: libloading::Library,
}

impl LoadedPlugin {
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
        id: req.id,
        format: std::ptr::null(),
        format_len: 0,
        payload: std::ptr::null_mut(),
        payload_len: 0,
    }
}

unsafe extern "C" fn noop_host_free_response(_: *mut AbiHost, _: AbiResponse) {}
