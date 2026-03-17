use super::{
    context::AbiCallContext,
    message::{AbiMessage, AbiOwnedMessage},
    types::AbiResultCode,
};

pub type FnCall = unsafe extern "C" fn(
    ctx: AbiCallContext,
    req: AbiMessage,
    out: *mut AbiOwnedMessage,
) -> AbiResultCode;

pub type FnFree = unsafe extern "C" fn(msg: *mut AbiOwnedMessage);