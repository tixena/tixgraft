//! Command-line interface module.
//!
//! Handles argument parsing and CLI commands.

#![expect(clippy::pub_use, reason = "deliberate module re-export for public API")]

pub mod args;

pub use args::*;
