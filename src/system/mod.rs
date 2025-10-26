//! System abstraction for environment and filesystem operations
//!
//! This module provides a unified trait for all external system interactions,
//! allowing for easy testing with mock implementations.

use std::env::VarError;
use std::fs::Metadata;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub mod mock;
pub mod real;

pub use mock::MockSystem;
pub use real::RealSystem;

/// Unified trait for system operations (environment + filesystem)
///
/// This trait abstracts all interactions with the operating system,
/// including environment variables and filesystem operations.
///
/// # Implementations
/// - `RealSystem`: Production implementation using `std::env` and `std::fs`
/// - `MockSystem`: Test implementation using in-memory storage
pub trait System: Send + Sync {
    // ==================== Environment Operations ====================

    /// Get an environment variable
    fn env_var(&self, key: &str) -> Result<String, VarError>;

    /// Get the current working directory
    fn current_dir(&self) -> io::Result<PathBuf>;

    // ==================== Filesystem Operations ====================

    /// Read entire file contents as a string
    fn read_to_string(&self, path: &Path) -> io::Result<String>;

    /// Write bytes to a file, creating it if it doesn't exist
    fn write(&self, path: &Path, contents: &[u8]) -> io::Result<()>;

    /// Recursively create a directory and all parent directories
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;

    /// Remove a directory and all its contents
    fn remove_dir_all(&self, path: &Path) -> io::Result<()>;

    /// Copy a file from source to destination
    fn copy(&self, from: &Path, to: &Path) -> io::Result<u64>;

    /// Check if a path exists
    fn exists(&self, path: &Path) -> bool;

    /// Check if a path points to a file
    fn is_file(&self, path: &Path) -> bool;

    /// Check if a path points to a directory
    fn is_dir(&self, path: &Path) -> bool;

    /// Get metadata for a path
    fn metadata(&self, path: &Path) -> io::Result<Metadata>;

    /// Canonicalize a path (resolve to absolute path)
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf>;

    /// Read directory entries, returning paths of all entries
    fn read_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>>;

    /// Open a file for reading (returns a readable stream)
    fn open(&self, path: &Path) -> io::Result<Box<dyn Read + '_>>;

    /// Create a file for writing (returns a writable stream)
    fn create(&self, path: &Path) -> io::Result<Box<dyn Write + '_>>;
}
