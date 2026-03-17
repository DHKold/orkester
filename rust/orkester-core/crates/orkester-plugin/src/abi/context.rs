use super::{host::AbiHostApi, types::AbiComponentHandle};

#[repr(C)]
#[derive(Copy, Clone)]
pub struct AbiCallContext {
    pub host: *const AbiHostApi,
    pub component: AbiComponentHandle, // null => root component
}

impl AbiCallContext {
    pub const fn new(host: *const AbiHostApi, component: AbiComponentHandle) -> Self {
        Self { host, component }
    }

    pub const fn root(host: *const AbiHostApi) -> Self {
        Self {
            host,
            component: core::ptr::null_mut(),
        }
    }
}