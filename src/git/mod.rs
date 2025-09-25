//! Git operations module
//! 
//! Handles Git repository operations including sparse checkout

pub mod sparse_checkout;
pub mod repository;

pub use sparse_checkout::*;
pub use repository::*;
