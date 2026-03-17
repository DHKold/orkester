use core::ffi::c_void;

use super::{
    message::{AbiMessage, AbiOwnedMessage},
    types::AbiResultCode,
};

pub type AbiHostCallFn = unsafe extern "C" fn(
    host_ctx: *mut c_void,
    req: AbiMessage,
    out: *mut AbiOwnedMessage,
) -> AbiResultCode;

pub type AbiHostFreeFn =
    unsafe extern "C" fn(host_ctx: *mut c_void, msg: *mut AbiOwnedMessage);

#[repr(C)]
#[derive(Clone, Debug)]
pub struct AbiHostApi {
    pub abi_version: u32,
    pub host_ctx: *mut c_void,
    pub call_host: AbiHostCallFn,
    pub free_host_message: AbiHostFreeFn,
}