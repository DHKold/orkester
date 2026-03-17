macro_rules! export_plugin {
    ($plugin_ty:ty) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn plugin_call(
            ctx: $crate::abi::AbiCallContext,
            req: $crate::abi::AbiMessage,
            out: *mut $crate::abi::AbiOwnedMessage,
        ) -> $crate::abi::AbiResultCode {
            $crate::private::dispatch_plugin_call::<$plugin_ty>(ctx, req, out)
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn plugin_free(
            msg: *mut $crate::abi::AbiOwnedMessage,
        ) {
            $crate::private::dispatch_plugin_free(msg);
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn plugin_init(
            host: *const $crate::abi::AbiHostApi,
        ) -> *mut core::ffi::c_void {
            match $crate::private::init_plugin_runtime::<$plugin_ty>(host) {
                Ok(ptr) => {
                    $crate::private::store_plugin_runtime::<$plugin_ty>(ptr);
                    ptr
                }
                Err(_) => core::ptr::null_mut(),
            }
        }
    };
}