//! Error handling for tixgraft.
//!
//! Re-exports `GraftError` from `types`, whose variants decide the process exit code.

#![expect(clippy::pub_use, reason = "deliberate module re-export for public API")]

pub mod types;

pub use types::*;
