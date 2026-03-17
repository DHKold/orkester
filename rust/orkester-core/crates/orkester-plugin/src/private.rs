use core::ffi::c_void;

use crate::{
    abi::{AbiCallContext, AbiHostApi, AbiMessage, AbiOwnedMessage, AbiResultCode},
    sdk::{Plugin, Result},
};

pub trait Sealed {}

pub fn init_plugin_runtime<P: Plugin>(host: *const AbiHostApi) -> Result<*mut c_void> {
    unsafe { crate::sdk::runtime::plugin_create_root::<P>(host) }
}

pub fn store_plugin_runtime<P: Plugin>(ptr: *mut c_void) {
    crate::sdk::runtime::set_plugin_runtime::<P>(ptr);
}

pub unsafe fn dispatch_plugin_call<P: Plugin>(
    ctx: AbiCallContext,
    req: AbiMessage,
    out: *mut AbiOwnedMessage,
) -> AbiResultCode {
    unsafe { crate::sdk::runtime::plugin_call::<P>(ctx, req, out) }
}

pub unsafe fn dispatch_plugin_free(msg: *mut AbiOwnedMessage) {
    unsafe { crate::sdk::runtime::plugin_free(msg) };
}