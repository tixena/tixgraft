//! Git operations module.
//!
//! Handles Git repository operations including sparse checkout.

#![expect(clippy::pub_use, reason = "deliberate module re-export for public API")]

pub mod repository;
pub mod sparse_checkout;

pub use repository::*;
pub use sparse_checkout::*;
