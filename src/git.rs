//! Repository handling and Git checkout.
//!
//! Re-exports the source classification from `repository` and the checkout from `sparse_checkout`.

#![expect(clippy::pub_use, reason = "deliberate module re-export for public API")]

pub mod repository;
pub mod sparse_checkout;

pub use repository::*;
pub use sparse_checkout::*;
