use crate::abi::AbiMessage;

use super::{Error, Result};

#[derive(Debug, Clone, Copy)]
pub struct Message<'a> {
    id: u64,
    type_id: u32,
    flags: u32,
    payload: &'a [u8],
}

impl<'a> Message<'a> {
    #[must_use]
    pub const fn new(id: u64, type_id: u32, flags: u32, payload: &'a [u8]) -> Self {
        Self {
            id,
            type_id,
            flags,
            payload,
        }
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
    pub const fn payload(&self) -> &'a [u8] {
        self.payload
    }

    pub fn utf8(&self) -> Result<&'a str> {
        core::str::from_utf8(self.payload).map_err(|_| Error::InvalidUtf8)
    }

    pub(crate) unsafe fn from_abi(raw: AbiMessage) -> Result<Self> {
        let payload = match (raw.payload.is_null(), raw.len) {
            (true, 0) => &[],
            (false, len) => unsafe { core::slice::from_raw_parts(raw.payload, len as usize) },
            (true, _) => return Err(Error::InvalidMessage),
        };

        Ok(Self {
            id: raw.id,
            type_id: raw.type_id,
            flags: raw.flags,
            payload,
        })
    }
}