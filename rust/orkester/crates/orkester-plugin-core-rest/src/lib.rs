mod root;
mod rest_server;

pub use root::RootComponent;

orkester_plugin::export_plugin_root_with_host!(RootComponent);
