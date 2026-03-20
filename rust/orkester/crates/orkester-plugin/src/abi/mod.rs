pub const ORKESTER_ABI_VERSION: u32 = 1;

#[repr(C)]
pub struct AbiComponent {
    // The orkester ABI protocol version that this component implements. This allows the host to ensure compatibility with the component.
    pub protocol: u32,

    // A pointer to the component's context, which can be used to store state or other data needed by the component.
    pub context: *mut std::ffi::c_void,

    // Methods for handling requests, freeing responses, and freeing the component itself.
    pub handle: unsafe extern "C" fn(this: *mut AbiComponent, req: AbiRequest) -> AbiResponse,
    pub free_response: unsafe extern "C" fn(this: *mut AbiComponent, res: AbiResponse),
    pub free: unsafe extern "C" fn(this: *mut AbiComponent),
}

#[repr(C)]
pub struct AbiRequest {
    // A unique identifier for this request, which can be used to correlate requests and responses.
    pub id: u64,

    /// The format field can be used to indicate the serialization format of the payload.
    /// The format string is not interpreted by the ABI itself but can be used by the host and plugin to agree on how to serialize and deserialize the payload bytes.
    /// 
    /// Examples:
    /// - `std/json` for JSON-encoded payloads
    /// - `custom-type1+UTF8` for a custom format defined by the plugin
    pub format: *const u8,
    pub format_len: u32,

    // A pointer to the request payload bytes, and its length. The component must not modify the bytes and should treat them as read-only.
    pub payload: *const u8,
    pub payload_len: u32,
}

#[repr(C)]
pub struct AbiResponse {
    // A unique identifier for this response, which can be used to correlate responses with requests.
    pub id: u64,

    /// The format field can be used to indicate the serialization format of the payload.
    /// The format string is not interpreted by the ABI itself but can be used by the host and plugin to agree on how to serialize and deserialize the payload bytes.
    /// 
    /// Examples:
    /// - `std/json` for JSON-encoded payloads
    /// - `custom-type1+UTF8` for a custom format defined by the plugin
    pub format: *const u8,
    pub format_len: u32,

    // A pointer to the response payload bytes, and its length. The sender must allocate this buffer and keeps ownership of it until the receiver calls the free function to release it.
    pub payload: *mut u8,
    pub payload_len: u32,
}

#[repr(C)]
pub struct AbiHost {
    // The orkester ABI protocol version that this host implements. This allows components to ensure compatibility with the host.
    pub protocol: u32,

    // A pointer to the host's context, which can be used to store state or other data needed by the host.
    pub context: *mut std::ffi::c_void,

    // Methods for handling requests from components and freeing responses.
    pub handle: unsafe extern "C" fn(this: *mut AbiHost, req: AbiRequest) -> AbiResponse,
    pub free_response: unsafe extern "C" fn(this: *mut AbiHost, res: AbiResponse),
}

// Type alias for the root component builder function that the plugin must export. The host will call this function to create the root component of the plugin, passing a pointer to the host's ABI struct.
pub type FnRootComponentBuilder = unsafe extern "C" fn(host: *mut AbiHost) -> *mut AbiComponent;
