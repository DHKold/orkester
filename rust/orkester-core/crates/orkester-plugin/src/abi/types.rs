use core::ffi::c_void;

pub type AbiResultCode = i32;
pub type AbiMessageId = u64;
pub type AbiTypeId = u32;
pub type AbiFlags = u32;
pub type AbiLength = u32;

pub type AbiComponentHandle = *mut c_void;