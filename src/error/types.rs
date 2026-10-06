//! Custom error types with exit codes.

use thiserror::Error;

/// Main error type for tixgraft operations.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum GraftError {
    /// Command Error - one or more commands failed.
    #[error("Command error: {message}")]
    Command { message: String },

    /// Configuration Error - missing or invalid configuration.
    #[error("Configuration error: {message}")]
    Configuration { message: String },

    /// Filesystem Error - file operation failed.
    #[error("Filesystem error: {message}")]
    Filesystem { message: String },

    /// Git Error - Git operation failed.
    #[error("Git error: {message}")]
    Git { message: String },

    /// Skill Error - skill management operation failed.
    #[error("Skill error: {message}")]
    Skill { message: String },

    /// Source Error - source path not found in repository.
    #[error("Source error: {message}")]
    Source { message: String },
}

impl GraftError {
    /// Create a command error.
    #[inline]
    pub fn command<S>(message: S) -> Self
    where
        S: Into<String>,
    {
        Self::Command {
            message: message.into(),
        }
    }

    /// Create a configuration error.
    #[inline]
    pub fn configuration<S>(message: S) -> Self
    where
        S: Into<String>,
    {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// Get the appropriate exit code for this error type.
    #[must_use]
    #[inline]
    pub const fn exit_code(&self) -> i32 {
        match *self {
            Self::Configuration { message: _ } => 1,
            Self::Source { message: _ } => 2,
            Self::Command { message: _ } => 3,
            Self::Git { message: _ } => 4,
            Self::Filesystem { message: _ } => 5,
            Self::Skill { message: _ } => 6,
        }
    }

    /// Create a filesystem error.
    #[inline]
    pub fn filesystem<S>(message: S) -> Self
    where
        S: Into<String>,
    {
        Self::Filesystem {
            message: message.into(),
        }
    }

    /// Create a source error.
    #[inline]
    pub fn from_source<S>(message: S) -> Self
    where
        S: Into<String>,
    {
        Self::Source {
            message: message.into(),
        }
    }

    /// Create a git error.
    #[inline]
    pub fn git<S>(message: S) -> Self
    where
        S: Into<String>,
    {
        Self::Git {
            message: message.into(),
        }
    }

    /// Create a skill error.
    #[inline]
    pub fn skill<S>(message: S) -> Self
    where
        S: Into<String>,
    {
        Self::Skill {
            message: message.into(),
        }
    }
}
