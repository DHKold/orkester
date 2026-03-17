use crate::abi::{AbiHostApi, AbiOwnedMessage};

use super::{Error, Message, OwnedMessage, Result};

#[derive(Debug, Clone, Copy)]
pub struct Host {
    raw: *const AbiHostApi,
}

impl Host {
    #[must_use]
    pub const fn new(raw: *const AbiHostApi) -> Self {
        Self { raw }
    }

    #[must_use]
    pub const fn as_ptr(&self) -> *const AbiHostApi {
        self.raw
    }

    pub fn call(&self, request: Message<'_>) -> Result<OwnedMessage> {
        if self.raw.is_null() {
            return Err(Error::NullHostApi);
        }

        let mut out = AbiOwnedMessage::empty();

        let rc = unsafe {
            ((*self.raw).call_host)(
                (*self.raw).host_ctx,
                crate::abi::AbiMessage {
                    id: request.id(),
                    type_id: request.type_id(),
                    flags: request.flags(),
                    payload: request.payload().as_ptr(),
                    len: request.payload().len() as u32,
                },
                &mut out,
            )
        };

        if rc != 0 {
            return Err(Error::HostCallFailed);
        }

        unsafe { OwnedMessage::from_abi(out) }
    }
}