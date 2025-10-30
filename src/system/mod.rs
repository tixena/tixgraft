//! System abstraction for environment and filesystem operations
//!
//! This module provides a unified trait for all external system interactions,
//! allowing for easy testing with mock implementations.

#![expect(clippy::module_name_repetitions)]
use std::env::VarError;
use std::fs::Metadata;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub mod mock;
pub mod real;


/// Entry from directory walking
#[derive(Debug, Clone)]
#[non_exhaustive]
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
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The environment variable cannot be retrieved
    fn env_var(&self, key: &str) -> Result<String, VarError>;

    /// Get the current working directory
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The current working directory cannot be retrieved
    fn current_dir(&self) -> io::Result<PathBuf>;

    // ==================== Filesystem Operations ====================

    /// Read entire file contents as a string
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be read
    fn read_to_string(&self, path: &Path) -> io::Result<String>;

    /// Write bytes to a file, creating it if it doesn't exist
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be written
    fn write(&self, path: &Path, contents: &[u8]) -> io::Result<()>;

    /// Recursively create a directory and all parent directories
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The directory cannot be created
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;

    /// Remove a directory and all its contents
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The directory cannot be removed
    fn remove_dir_all(&self, path: &Path) -> io::Result<()>;

    /// Remove a file
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be removed
    fn remove_file(&self, path: &Path) -> io::Result<()>;

    /// Copy a file from source to destination
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be copied
    fn copy(&self, from: &Path, to: &Path) -> io::Result<u64>;

    /// Check if a path exists
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path cannot be checked
    fn exists(&self, path: &Path) -> io::Result<bool>;

    /// Check if a path points to a file
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path cannot be checked
    fn is_file(&self, path: &Path) -> io::Result<bool>;

    /// Check if a path points to a directory
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path cannot be checked
    fn is_dir(&self, path: &Path) -> io::Result<bool>;

    /// Get metadata for a path
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path cannot be read
    fn metadata(&self, path: &Path) -> io::Result<Metadata>;

    /// Canonicalize a path (resolve to absolute path)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path cannot be canonicalized
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf>;

    /// Read directory entries, returning paths of all entries
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The directory cannot be read
    fn read_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>>;

    /// Open a file for reading (returns a readable stream)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be opened
    fn open(&self, path: &Path) -> io::Result<Box<dyn Read + '_>>;

    /// Create a file for writing (returns a writable stream)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be created
    fn create(&self, path: &Path) -> io::Result<Box<dyn Write + '_>>;

    /// Create a temporary directory that is automatically cleaned up on drop
    ///
    /// # Returns
    /// A handle to the temporary directory. The directory will be removed when
    /// the handle is dropped.
    ///
    /// # Note
    /// For `RealSystem`, this uses `tempfile::TempDir` on the real filesystem.
    /// For `MockSystem`, this creates an in-memory temporary directory.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The temporary directory cannot be created
    fn create_temp_dir(&self) -> io::Result<Box<dyn TempDirHandle>>;

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
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The directory cannot be walked
    fn walk_dir(&self, path: &Path, follow_links: bool, hidden: bool)
    -> io::Result<Vec<WalkEntry>>;
}
