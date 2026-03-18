//! # orkester-plugin-test
//!
//! A demonstration [`cdylib`] plugin for the Orkester plugin SDK.
//! Shows how the high-level SDK reduces plugin authoring to plain Rust structs.
//!
//! ## Component catalogue
//!
//! | Kind        |  ID | Description                                           |
//! |-------------|----:|-------------------------------------------------------|
//! | Root        |   1 | Entry point; creates child components                 |
//! | Echo        |  10 | Reflects any payload back unchanged (format-agnostic) |
//! | Counter     |  11 | Stateful `i64` counter: Inc / Dec / Get / Reset       |
//! | Greeter     |  12 | JSON greeter (multi-language) + host log callback     |
//! | Calculator  |  13 | Binary arithmetic: add / sub / mul / div / pow / rem  |

mod components;
mod constants;

use orkester_plugin::declare_plugin;

// Emit the `orkester_create_root` symbol.  The closure receives the raw
// `*mut abi::Host` pointer and must return a `ComponentHandler`.
declare_plugin!(|host| components::Root::new(host));
