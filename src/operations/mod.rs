//! Operations module
//!
//! Coordinates pull operations including file copying, text replacement, and command execution

pub mod commands;
pub mod copy;
pub mod discovery;
pub mod post_commands;
pub mod pull;
pub mod replace;
pub mod to_command_line;
pub mod to_config;

pub use commands::*;
pub use copy::*;
pub use pull::*;
pub use replace::*;
pub use to_command_line::*;
pub use to_config::*;
