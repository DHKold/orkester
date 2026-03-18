use std::marker::PhantomData;

use crate::abi;
use super::constants::MSG_TYPE_POINTER;
use super::error::Error;
use super::message::{ComponentResponse, Request};

/// A safe, owned wrapper around a plugin-allocated `*mut abi::Component`.
///
/// When dropped, the component is freed by calling its own `free` function
/// pointer, which hands control back to the plugin's allocator.
///
/// # Thread safety
/// Plugins are required to make all component operations thread-safe.
/// `Component` therefore implements both `Send` and `Sync`.
pub struct Component {
    raw: *mut abi::Component,
}

// SAFETY: plugins must guarantee thread-safe component access.
unsafe impl Send for Component {}
unsafe impl Sync for Component {}

impl Component {
    /// Wrap a raw pointer returned by a plugin.
    ///
    /// # Errors
    /// Returns [`Error::NullComponent`] if `raw` is null.
    ///
    /// # Safety
    /// `raw` must be a valid, non-null pointer to a plugin-owned component that
    /// has not yet been freed.
    pub(crate) unsafe fn from_raw(raw: *mut abi::Component) -> Result<Self, Error> {
        if raw.is_null() {
            Err(Error::NullComponent)
        } else {
            Ok(Self { raw })
        }
    }

    /// The component's numeric ID as assigned by the plugin.
    ///
    /// The meaning is plugin-defined.  `0` conventionally denotes the root.
    pub fn id(&self) -> u32 {
        unsafe { (*self.raw).id }
    }

    /// The kind code that identifies this component's role.
    ///
    /// Compare against the `COMPONENT_KIND_*` constants or the IDs returned
    /// by [`ComponentKind`](super::metadata::ComponentKind).
    pub fn kind(&self) -> u32 {
        unsafe { (*self.raw).kind }
    }

    /// The ID of the parent component (`0` for the root component).
    pub fn parent(&self) -> u32 {
        unsafe { (*self.raw).parent }
    }

    /// The protocol version the component was compiled against.
    pub fn protocol(&self) -> u32 {
        unsafe { (*self.raw).protocol }
    }

    /// Send `request` to this component and receive the response.
    ///
    /// The returned [`ComponentResponse`] borrows from `self` because the
    /// plugin frees the response payload through this component's
    /// `free_response` pointer.  The response must not outlive `self`.
    pub fn handle(&self, request: Request) -> ComponentResponse<'_> {
        let abi_req = request.as_abi();
        // SAFETY: `raw` is a valid, live component; `abi_req` borrows from
        // `request` which is alive for the duration of this call.
        let response = unsafe {
            let handle_fn = (*self.raw).handle;
            handle_fn(self.raw, abi_req)
        };
        ComponentResponse {
            inner: response,
            component: self.raw,
            _lifetime: PhantomData,
        }
    }

    /// Ask this component to create a child component and return it as an
    /// owned [`Component`].
    ///
    /// The plugin must respond with [`MSG_TYPE_POINTER`]: a payload whose bytes
    /// are the little-endian representation of a `*mut abi::Component` pointer
    /// to the newly allocated child.
    ///
    /// # Errors
    /// - [`Error::UnexpectedFormat`] — response format was not `MSG_TYPE_POINTER`.
    /// - [`Error::InvalidPointerPayload`] — payload length ≠ pointer size.
    /// - [`Error::NullComponent`] — pointer decoded to null.
    pub fn create_component(&self, request: Request) -> Result<Component, Error> {
        let response = self.handle(request);

        if response.format() != MSG_TYPE_POINTER {
            return Err(Error::UnexpectedFormat {
                expected: MSG_TYPE_POINTER,
                got: response.format(),
            });
        }

        let bytes = response.as_bytes();
        let ptr_size = std::mem::size_of::<*mut abi::Component>();
        if bytes.len() != ptr_size {
            return Err(Error::InvalidPointerPayload);
        }

        // SAFETY: we checked the byte length matches a pointer, so this
        // bitwise copy reconstructs the address the plugin encoded.
        let raw: *mut abi::Component = unsafe {
            let mut ptr = std::ptr::null_mut::<abi::Component>();
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                &mut ptr as *mut _ as *mut u8,
                ptr_size,
            );
            ptr
        };

        // SAFETY: the plugin promises the pointer is valid and non-null.
        unsafe { Component::from_raw(raw) }
    }
}

impl Drop for Component {
    fn drop(&mut self) {
        // SAFETY: `raw` is the valid pointer we received from the plugin;
        // single ownership guarantees this runs exactly once.
        unsafe {
            let free_fn = (*self.raw).free;
            free_fn(self.raw);
        }
    }
}
