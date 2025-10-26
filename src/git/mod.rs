//! Git operations module
//!
//! Handles Git repository operations including sparse checkout

pub mod repository;
pub mod sparse_checkout;

pub use repository::*;
pub use sparse_checkout::*;
