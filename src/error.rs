//! Error handling module.
//!
//! Defines custom error types with appropriate exit codes.

#![expect(clippy::pub_use, reason = "deliberate module re-export for public API")]

pub mod types;

pub use types::*;
