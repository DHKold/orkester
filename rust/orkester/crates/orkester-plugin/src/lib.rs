pub mod abi;
pub mod sdk;
pub mod prelude;

// Re-export the proc macro so users can write `use orkester_plugin::prelude::*`
// and get `#[component]` in scope without a separate `extern crate orkester_macro`.
pub use orkester_macro::component;

/// Generate the C entry point for a plugin.
///
/// The type passed must implement [`sdk::PluginComponent`] and [`Default`].
///
/// ```ignore
/// orkester_plugin::export_plugin_root!(my_crate::components::RootComponent);
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
