pub mod abi;
pub mod hub;
pub mod sdk;
pub mod prelude;

// Re-export the proc macro so users can write `use orkester_plugin::prelude::*`
// and get `#[component]` in scope without a separate `extern crate orkester_macro`.
pub use orkester_macro::component;

/// Generate the C entry point for a plugin whose root component implements
/// [`sdk::PluginComponent`] and [`Default`].
///
/// ```ignore
/// orkester_plugin::export_plugin_root!(my_crate::RootComponent);
/// ```
#[macro_export]
macro_rules! export_plugin_root {
    ($ty:ty) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orkester_plugin_entry(
            _host: *mut $crate::abi::AbiHost,
        ) -> *mut $crate::abi::AbiComponent {
            use $crate::sdk::PluginComponent as _;
            let component = <$ty as ::std::default::Default>::default();
            ::std::boxed::Box::into_raw(::std::boxed::Box::new(
                <$ty as $crate::sdk::PluginComponent>::to_abi(component),
            ))
        }
    };
}

/// Like [`export_plugin_root!`] but passes the raw `*mut AbiHost` to
/// `<Type>::new(host)` instead of calling `Default::default()`.
///
/// The root component type must expose:
/// ```ignore
/// fn new(host: *mut orkester_plugin::abi::AbiHost) -> Self
/// ```
///
/// Use this variant when any child component created by the root needs to call
/// back to the host (e.g. a RestServer routing through the host dispatcher).
///
/// ```ignore
/// orkester_plugin::export_plugin_root_with_host!(my_crate::RootComponent);
/// ```
#[macro_export]
macro_rules! export_plugin_root_with_host {
    ($ty:ty) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orkester_plugin_entry(
            host: *mut $crate::abi::AbiHost,
        ) -> *mut $crate::abi::AbiComponent {
            use $crate::sdk::PluginComponent as _;
            let component = <$ty>::new(host);
            ::std::boxed::Box::into_raw(::std::boxed::Box::new(
                <$ty as $crate::sdk::PluginComponent>::to_abi(component),
            ))
        }
    };
}
