use crate::config::ConfigTree;

use orkester_common::log_info;
use orkester_common::logging::{consumers::ConsoleConsumer, Logger};

pub(crate) fn init_logging() {
    Logger::add_consumer(ConsoleConsumer);
}

/// Load additional logging configuration from the config tree, if present.
pub(crate) fn load_logging_config(config_tree: &ConfigTree) {
    if let Some(_log_config) = config_tree.get("logging.fileConsumer") {
        // TODO: implement dynamic file consumer config parsing and creation
        log_info!("File consumer config found, but dynamic loading is not implemented yet.");
    }
}
