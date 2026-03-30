//! S3 document loader — loads documents from S3/MinIO object storage.
//!
//! | Module       | Responsibility                                       |
//! |--------------|------------------------------------------------------|
//! | `types`      | Config, runtime state, change events, metrics        |
//! | `auth`       | AWS Signature V4 signing                             |
//! | `client`     | HTTP: list objects, get object content               |
//! | `scanner`    | Scan bucket prefix, produce change events            |
//! | `watcher`    | Background polling thread                            |
//! | `loader`     | `S3Loader` struct (start, load, add_entry)           |
//! | `component`  | Orkester component wrapper                           |

pub mod auth;
pub mod client;
pub mod component;
pub mod loader;
pub mod scanner;
pub mod types;
pub mod watcher;

pub use component::S3LoaderComponent;
pub use loader::S3Loader;
pub use types::{S3ChangeEvent, S3Entry, S3LoaderConfig, S3LoaderEntryConfig, S3ScanMetrics};
