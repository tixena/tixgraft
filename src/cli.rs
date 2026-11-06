//! Command-line interface of tixgraft.
//!
//! Re-exports from `args` the clap argument types and the configuration types of a pull.

#![expect(clippy::pub_use, reason = "deliberate module re-export for public API")]

pub mod args;

pub use args::*;
