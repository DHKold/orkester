mod metrics;
mod root;

pub use root::MetricsRoot;

orkester_plugin::export_plugin_root!(MetricsRoot);
