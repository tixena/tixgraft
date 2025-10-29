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

/// Entry from directory walking
#[derive(Debug, Clone)]
pub struct WalkEntry {
    pub path: PathBuf,
    pub is_file: bool,
    pub is_dir: bool,
}

/// Temporary directory handle that cleans up on drop
///
/// This trait provides a system-agnostic way to create temporary directories
/// that are automatically cleaned up when the handle is dropped.
///
/// For `RealSystem`, this wraps `tempfile::TempDir` and uses real filesystem.
/// For `MockSystem`, this manages an in-memory temporary directory.
pub trait TempDirHandle {
    /// Get the path to the temporary directory
    fn path(&self) -> &Path;
}

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

    /// Remove a file
    fn remove_file(&self, path: &Path) -> io::Result<()>;

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

    /// Recursively walk a directory, returning all entries
    ///
    /// # Arguments
    /// * `path` - Root path to start walking from
    /// * `follow_links` - Whether to follow symbolic links
    /// * `hidden` - Whether to include hidden files
    ///
    /// # Returns
    /// Vector of all entries found (files and directories), excluding the root itself
    ///
    /// # Note
    /// For `RealSystem`, this respects .gitignore files using the `ignore` crate.
    /// For `MockSystem`, this walks the in-memory filesystem.
    fn walk_dir(&self, path: &Path, follow_links: bool, hidden: bool)
    -> io::Result<Vec<WalkEntry>>;

    /// Create a temporary directory that is automatically cleaned up on drop
    ///
    /// # Returns
    /// A handle to the temporary directory. The directory will be removed when
    /// the handle is dropped.
    ///
    /// # Note
    /// For `RealSystem`, this uses `tempfile::TempDir` on the real filesystem.
    /// For `MockSystem`, this creates an in-memory temporary directory.
    fn create_temp_dir(&self) -> io::Result<Box<dyn TempDirHandle>>;
}
