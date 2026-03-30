mod rest_server;
mod root;

pub use root::RootComponent;

// Use the host-pointer variant so child factories can receive *mut AbiHost.
orkester_plugin::export_plugin_root_with_host!(RootComponent);

