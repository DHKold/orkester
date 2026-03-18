#[repr(C)]
pub struct Component {
    pub protocol: u32,
    pub id: u32,
    pub kind: u32,
    pub parent: u32,
    /// Opaque handler state managed by the SDK or the plugin directly.
    /// The SDK stores a `*mut H` (the [`ComponentHandler`] impl) here;
    /// raw plugin implementations may use it for any purpose they choose.
    ///
    /// [`ComponentHandler`]: crate::sdk::ComponentHandler
    pub context: *mut std::ffi::c_void,
    pub handle: unsafe extern "C" fn(this: *mut Component, req: Request) -> Response,
    pub free_response: unsafe extern "C" fn(this: *mut Component, res: Response),
    pub free: unsafe extern "C" fn(this: *mut Component),
}

#[repr(C)]
pub struct Request {
    pub id: u64,
    pub format: u32,
    pub flags: u32,
    pub payload: *const u8,
    pub len: u32,
}

#[repr(C)]
pub struct Response {
    pub id: u64,
    pub format: u32,
    pub flags: u32,
    pub payload: *mut u8,
    pub len: u32,
}

#[repr(C)]
pub struct Host {
    pub protocol: u32,
    /// Opaque user-data pointer.  The host sets this to whatever state its
    /// callback functions need; the plugin must treat it as an opaque value
    /// and must not read, write, or free it.
    pub context: *mut std::ffi::c_void,
    pub handle: unsafe extern "C" fn(this: *mut Host, req: Request) -> Response,
    pub free_response: unsafe extern "C" fn(this: *mut Host, res: Response),
}

/// Signature of the symbol every plugin must export under the name
/// `orkester_create_root`.
///
/// The host passes a stable pointer to its [`Host`] struct.  The plugin may
/// store the pointer and call back through it at any time until it returns
/// from `free` on the root component.
pub type FnComponentBuilder = unsafe extern "C" fn(host: *mut Host) -> *mut Component;