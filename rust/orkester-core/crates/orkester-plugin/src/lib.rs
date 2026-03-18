pub mod abi;
pub mod sdk;

/// Generate the `orkester_create_root` symbol that every Orkester plugin must
/// export.
///
/// # Usage
/// ```ignore
/// declare_plugin!(MyRootHandler);
/// ```
///
/// Where `MyRootHandler` is a type that implements
/// [`sdk::ComponentHandler`] and can be constructed with `Default`.
///
/// The macro produces:
/// ```ignore
/// #[unsafe(no_mangle)]
/// pub unsafe extern "C" fn orkester_create_root(
///     host: *mut abi::Host,
/// ) -> *mut abi::Component { … }
/// ```
///
/// The root component will have `id = 0`, `kind = COMPONENT_KIND_PLUGIN`,
/// `parent = 0`.
#[macro_export]
macro_rules! declare_plugin {
    ($root:expr) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orkester_create_root(
            host: *mut $crate::abi::Host,
        ) -> *mut $crate::abi::Component {
            $crate::sdk::alloc_component(
                0,
                $crate::sdk::COMPONENT_KIND_PLUGIN,
                0,
                $root(host),
            )
        }
    };
}
