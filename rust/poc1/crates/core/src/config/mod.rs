//! Config module: main loader and API

mod access;
mod interface;
mod json_loader;
mod loader;
mod toml_loader;
mod yaml_loader;

// Public API
pub use access::ConfigTree;
pub use loader::load_config_files;
