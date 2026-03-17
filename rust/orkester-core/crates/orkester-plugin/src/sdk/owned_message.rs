use crate::abi::{AbiMessage, AbiOwnedMessage};

use super::{Error, Result};

#[derive(Debug, Clone)]
pub struct OwnedMessage {
    id: u64,
    type_id: u32,
    flags: u32,
    payload: Vec<u8>,
}

impl OwnedMessage {
    #[must_use]
    pub fn new(id: u64, type_id: u32, flags: u32, payload: Vec<u8>) -> Self {
        Self {
            id,
            type_id,
            flags,
            payload,
        }
    }

    #[must_use]
    pub fn empty(id: u64, type_id: u32, flags: u32) -> Self {
        Self::new(id, type_id, flags, Vec::new())
    }

    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    #[must_use]
    pub const fn type_id(&self) -> u32 {
        self.type_id
    }

    #[must_use]
    pub const fn flags(&self) -> u32 {
        self.flags
    }

    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    #[must_use]
    pub fn payload_mut(&mut self) -> &mut Vec<u8> {
        &mut self.payload
    }

    pub fn utf8(&self) -> Result<&str> {
        core::str::from_utf8(&self.payload).map_err(|_| Error::InvalidUtf8)
    }

    #[must_use]
    pub fn as_message(&self) -> AbiMessage {
        AbiMessage {
            id: self.id,
            type_id: self.type_id,
            flags: self.flags,
            payload: self.payload.as_ptr(),
            len: self.payload.len() as u32,
        }
    }

    pub(crate) fn into_abi(mut self) -> AbiOwnedMessage {
        let raw = AbiOwnedMessage {
            id: self.id,
            type_id: self.type_id,
            flags: self.flags,
            payload: self.payload.as_mut_ptr(),
            len: self.payload.len() as u32,
        };

        core::mem::forget(self.payload);
        raw
    }

    pub(crate) unsafe fn from_abi(raw: AbiOwnedMessage) -> Result<Self> {
        let payload = match (raw.payload.is_null(), raw.len) {
            (true, 0) => Vec::new(),
            (false, len) => unsafe { Vec::from_raw_parts(raw.payload, len as usize, len as usize) },
            (true, _) => return Err(Error::InvalidOwnedMessage),
        };

        Ok(Self {
            id: raw.id,
            type_id: raw.type_id,
            flags: raw.flags,
            payload,
        })
    }
}