use core::{
    ffi::c_void,
    sync::atomic::{AtomicPtr, Ordering},
};

use crate::abi::{AbiCallContext, AbiHostApi, AbiMessage, AbiOwnedMessage, AbiResultCode};

use super::{Component, Host, Message, OwnedMessage, Plugin, Result};

pub(crate) struct Runtime<P: Plugin> {
    pub(crate) host: Host,
    pub(crate) plugin: P,
}

pub(crate) struct ComponentBox {
    pub(crate) inner: Box<dyn Component>,
}

fn runtime_cell() -> &'static AtomicPtr<c_void> {
    static CELL: AtomicPtr<c_void> = AtomicPtr::new(core::ptr::null_mut());
    &CELL
}

pub(crate) fn set_plugin_runtime<P: Plugin>(ptr: *mut c_void) {
    let _ = core::marker::PhantomData::<P>;
    runtime_cell().store(ptr, Ordering::Release);
}

fn get_plugin_runtime<P: Plugin>() -> *mut c_void {
    let _ = core::marker::PhantomData::<P>;
    runtime_cell().load(Ordering::Acquire)
}

pub(crate) unsafe fn plugin_create_root<P: Plugin>(host: *const AbiHostApi) -> Result<*mut c_void> {
    let host = Host::new(host);
    let plugin = P::new(host)?;
    let runtime = Box::new(Runtime { host, plugin });
    Ok(Box::into_raw(runtime) as *mut c_void)
}

pub(crate) fn write_output(out: *mut AbiOwnedMessage, message: OwnedMessage) -> AbiResultCode {
    if out.is_null() {
        return 1;
    }

    unsafe {
        *out = message.into_abi();
    }

    0
}

pub(crate) fn protect<F>(f: F) -> AbiResultCode
where
    F: FnOnce() -> AbiResultCode,
{
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(code) => code,
        Err(_) => 1,
    }
}

pub(crate) unsafe extern "C" fn plugin_call<P: Plugin>(
    ctx: AbiCallContext,
    req: AbiMessage,
    out: *mut AbiOwnedMessage,
) -> AbiResultCode {
    protect(|| {
        let request = unsafe {
            match Message::from_abi(req) {
                Ok(value) => value,
                Err(_) => return 1,
            }
        };

        if ctx.component.is_null() {
            let runtime_ptr = get_plugin_runtime::<P>();
            if runtime_ptr.is_null() {
                return 1;
            }

            let runtime = unsafe { &mut *(runtime_ptr as *mut Runtime<P>) };

            return match runtime.plugin.handle(request) {
                Ok(message) => write_output(out, message),
                Err(_) => 1,
            };
        }

        let component = unsafe { &mut *(ctx.component as *mut ComponentBox) };

        match component.inner.handle(Host::new(ctx.host), request) {
            Ok(message) => write_output(out, message),
            Err(_) => 1,
        }
    })
}

pub(crate) unsafe extern "C" fn plugin_free(msg: *mut AbiOwnedMessage) {
    if msg.is_null() {
        return;
    }

    let raw = unsafe { core::ptr::read(msg) };
    let _ = unsafe { OwnedMessage::from_abi(raw) };

    unsafe {
        *msg = AbiOwnedMessage::empty();
    }
}

pub(crate) fn create_component_box(component: Box<dyn Component>) -> OwnedMessage {
    let handle = Box::into_raw(Box::new(ComponentBox { inner: component })) as *mut c_void;
    let payload = (handle as usize).to_ne_bytes().to_vec();

    OwnedMessage::new(0, crate::abi::TYPE_COMPONENT, crate::abi::FLAG_NONE, payload)
}