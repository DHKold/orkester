use super::types::{AbiFlags, AbiLength, AbiMessageId, AbiTypeId};

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct AbiMessage {
    pub id: AbiMessageId,
    pub type_id: AbiTypeId,
    pub flags: AbiFlags,
    pub payload: *const u8,
    pub len: AbiLength,
}

impl AbiMessage {
    pub const fn new(
        id: AbiMessageId,
        type_id: AbiTypeId,
        flags: AbiFlags,
        payload: *const u8,
        len: AbiLength,
    ) -> Self {
        Self {
            id,
            type_id,
            flags,
            payload,
            len,
        }
    }

    pub const fn empty(id: AbiMessageId, type_id: AbiTypeId, flags: AbiFlags) -> Self {
        Self {
            id,
            type_id,
            flags,
            payload: core::ptr::null(),
            len: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct AbiOwnedMessage {
    pub id: AbiMessageId,
    pub type_id: AbiTypeId,
    pub flags: AbiFlags,
    pub payload: *mut u8,
    pub len: AbiLength,
}

impl AbiOwnedMessage {
    pub const fn new(
        id: AbiMessageId,
        type_id: AbiTypeId,
        flags: AbiFlags,
        payload: *mut u8,
        len: AbiLength,
    ) -> Self {
        Self {
            id,
            type_id,
            flags,
            payload,
            len,
        }
    }

    pub const fn empty() -> Self {
        Self {
            id: 0,
            type_id: 0,
            flags: 0,
            payload: core::ptr::null_mut(),
            len: 0,
        }
    }
}