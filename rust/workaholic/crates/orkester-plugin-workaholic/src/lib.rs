pub mod catalog_server;
pub mod host_client;
pub mod root;
pub mod task_runner;
pub mod worker;
pub mod workflow_server;

use root::RootComponent;
orkester_plugin::export_plugin_root_with_host!(RootComponent);
