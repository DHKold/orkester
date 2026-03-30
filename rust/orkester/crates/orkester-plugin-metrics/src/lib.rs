mod metrics;
mod root;

pub use root::MetricsPlugin;

orkester_plugin::export_plugin_root!(MetricsPlugin);
