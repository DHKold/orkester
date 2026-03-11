//! Config module: main loader and API

mod interface;
mod json_loader;
mod yaml_loader;
mod toml_loader;
mod access;
mod loader;

// Public API
pub use interface::ConfigLoader;
pub use access::ConfigTree;
pub use loader::{load_config_files, extract_logging_config};
