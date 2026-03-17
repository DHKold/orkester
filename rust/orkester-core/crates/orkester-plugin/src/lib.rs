#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(elided_lifetimes_in_paths)]
#![deny(unused_lifetimes)]
#![deny(unused_attributes)]
#![deny(rust_2018_idioms)]
#![deny(rust_2021_prelude_collisions)]
#![deny(missing_debug_implementations)]

pub mod abi;
pub mod sdk;

mod macros;

#[doc(hidden)]
pub mod __private;