pub mod executor;

use executor::KubernetesExecutorBuilder;
use orkester_common::logging::Logger;
use orkester_common::plugin::{ComponentMetadata, Plugin, PluginComponent, PluginMetadata};

/// Constructs the Kubernetes plugin with all its components.
pub fn k8s_plugin() -> Plugin {
    Plugin {
        metadata: PluginMetadata {
            id: "orkester-plugin-k8s".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: "Kubernetes executor: runs Orkester tasks as ephemeral Pods.".to_string(),
            authors: vec!["Orkester Contributors".to_string()],
        },
        components: vec![ComponentMetadata {
            kind: "executor".to_string(),
            id: "kubernetes".to_string(),
            description: "Runs tasks as ephemeral Kubernetes Pods; captures stdout outputs via \
                          the sentinel pattern."
                .to_string(),
            builder: PluginComponent::ExecutorProvider(Box::new(KubernetesExecutorBuilder)),
        }],
    }
}

/// Well-known dynamic-loading entry point.
///
/// # Safety
/// The returned pointer is heap-allocated and ownership is transferred to the caller.
#[no_mangle]
pub extern "C" fn orkester_register_plugin() -> *mut Plugin {
    Box::into_raw(Box::new(k8s_plugin()))
}

/// Logger-injection entry point called by Orkester right after loading this library.
///
/// # Safety
/// `logger` must be a valid pointer to a [`Logger`] that lives for the entire
/// process lifetime (i.e. the host's `Logger::global()`).
#[no_mangle]
pub unsafe extern "C" fn orkester_set_logger(logger: *const Logger) {
    Logger::inject(logger);
}
