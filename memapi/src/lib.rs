#![deny(unsafe_op_in_unsafe_fn)]
pub mod ffi;
pub mod flamegraph;
pub mod memorytracking;
pub mod mmap;
pub mod oom;
mod python;
mod rangemap;
pub mod util;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate derivative;
