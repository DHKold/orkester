mod formatter;
mod root;
mod server;
mod sink;

pub use root::LoggingRoot;

orkester_plugin::export_plugin_root_with_host!(LoggingRoot);
