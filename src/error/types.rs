//! The error type of tixgraft operations.
//!
//! Each variant maps to the process exit code the binary ends with.

use thiserror::Error;

/// Error of a tixgraft operation; `message` in every variant is the text shown to the user.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum GraftError {
    /// A command run after copying was empty or failed; exits with code 3.
    #[error("Command error: {message}")]
    Command { message: String },

    /// The configuration is missing or invalid; exits with code 1.
    #[error("Configuration error: {message}")]
    Configuration { message: String },

    /// A file operation failed; exits with code 5.
    #[error("Filesystem error: {message}")]
    Filesystem { message: String },

    /// A Git operation failed; exits with code 4.
    #[error("Git error: {message}")]
    Git { message: String },

    /// A skill management operation failed; exits with code 6.
    #[error("Skill error: {message}")]
    Skill { message: String },

    /// The source path or local repository is missing or of the wrong kind; exits with code 2.
    #[error("Source error: {message}")]
    Source { message: String },
}

impl GraftError {
    /// Wraps `message` in the `Command` variant.
    #[inline]
    pub fn command<S>(message: S) -> Self
    where
        S: Into<String>,
    {
        Self::Command {
            message: message.into(),
        }
    }

    /// Wraps `message` in the `Configuration` variant.
    #[inline]
    pub fn configuration<S>(message: S) -> Self
    where
        S: Into<String>,
    {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// Returns the process exit code of this variant, from 1 to 6.
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

    /// Wraps `message` in the `Filesystem` variant.
    #[inline]
    pub fn filesystem<S>(message: S) -> Self
    where
        S: Into<String>,
    {
        Self::Filesystem {
            message: message.into(),
        }
    }

    /// Wraps `message` in the `Source` variant.
    #[inline]
    pub fn from_source<S>(message: S) -> Self
    where
        S: Into<String>,
    {
        Self::Source {
            message: message.into(),
        }
    }

    /// Wraps `message` in the `Git` variant.
    #[inline]
    pub fn git<S>(message: S) -> Self
    where
        S: Into<String>,
    {
        Self::Git {
            message: message.into(),
        }
    }

    /// Wraps `message` in the `Skill` variant.
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
