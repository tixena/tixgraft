//! Custom error types with exit codes

use thiserror::Error;

/// Main error type for tixgraft operations
#[derive(Error, Debug)]
pub enum GraftError {
    /// Configuration Error - missing or invalid configuration
    #[error("Configuration error: {message}")]
    Configuration { message: String },

    /// Source Error - source path not found in repository
    #[error("Source error: {message}")]
    Source { message: String },

    /// Command Error - one or more commands failed
    #[error("Command error: {message}")]
    Command { message: String },

    /// Git Error - Git operation failed
    #[error("Git error: {message}")]
    Git { message: String },

    /// Filesystem Error - file operation failed
    #[error("Filesystem error: {message}")]
    Filesystem { message: String },
}

impl GraftError {
    /// Get the appropriate exit code for this error type
    #[must_use]
    pub const fn exit_code(&self) -> i32 {
        match self {
            Self::Configuration { .. } => 1,
            Self::Source { .. } => 2,
            Self::Command { .. } => 3,
            Self::Git { .. } => 4,
            Self::Filesystem { .. } => 5,
        }
    }

    /// Create a configuration error
    pub fn configuration<S: Into<String>>(message: S) -> Self {
        return Self::Configuration {
            message: message.into(),
        };
    }

    /// Create a source error
    pub fn source<S: Into<String>>(message: S) -> Self {
        return Self::Source {
            message: message.into(),
        };
    }

    /// Create a command error
    pub fn command<S: Into<String>>(message: S) -> Self {
        return Self::Command {
            message: message.into(),
        };
    }

    /// Create a git error
    pub fn git<S: Into<String>>(message: S) -> Self {
        return Self::Git {
            message: message.into(),
        };
    }

    /// Create a filesystem error
    pub fn filesystem<S: Into<String>>(message: S) -> Self {
        return Self::Filesystem {
            message: message.into(),
        };
    }
}
