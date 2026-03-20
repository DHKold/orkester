use crate::abi::AbiComponent;
use super::metadata::ComponentMetadata;

/// Trait that every plugin component must implement.
pub trait PluginComponent: Sized {
    /// Static metadata describing this component kind.
    fn get_metadata() -> ComponentMetadata;

    /// Consume `self` and produce a stable ABI vtable.
    ///
    /// Implementors should use [`crate::sdk::AbiHandlerBuilder`] to construct
    /// the return value.
    fn to_abi(self) -> AbiComponent;
}
