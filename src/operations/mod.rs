//! Operations module
//! 
//! Coordinates pull operations including file copying, text replacement, and command execution

pub mod pull;
pub mod copy;
pub mod replace;
pub mod commands;

pub use pull::*;
pub use copy::*;
pub use replace::*;
pub use commands::*;
