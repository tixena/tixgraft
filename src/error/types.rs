//! Custom error types with exit codes

use thiserror::Error;

/// Main error type for tixgraft operations
#[derive(Error, Debug)]
#[non_exhaustive]
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
    #[inline]
    pub const fn exit_code(&self) -> i32 {
        match *self {
            Self::Configuration { .. } => 1,
            Self::Source { .. } => 2,
            Self::Command { .. } => 3,
            Self::Git { .. } => 4,
            Self::Filesystem { .. } => 5,
        }
    }

    /// Create a configuration error
    #[inline]
    pub fn configuration<S: Into<String>>(message: S) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// Create a source error
    #[inline]
    pub fn from_source<S: Into<String>>(message: S) -> Self {
        Self::Source {
            message: message.into(),
        }
    }

    /// Create a command error
    #[inline]
    pub fn command<S: Into<String>>(message: S) -> Self {
        Self::Command {
            message: message.into(),
        }
    }

    /// Create a git error
    #[inline]
    pub fn git<S: Into<String>>(message: S) -> Self {
        Self::Git {
            message: message.into(),
        }
    }

    /// Create a filesystem error
    #[inline]
    pub fn filesystem<S: Into<String>>(message: S) -> Self {
        Self::Filesystem {
            message: message.into(),
        }
    }
}
