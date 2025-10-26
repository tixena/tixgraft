//! Mock system implementation for testing

use super::System;
use std::collections::{HashMap, HashSet};
use std::env::VarError;
use std::fs::Metadata;
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// In-memory implementation of System trait for testing
///
/// `MockSystem` provides an in-memory filesystem and environment,
/// perfect for fast, isolated unit tests without side effects.
///
/// # Example
/// ```
/// use tixgraft::system::{MockSystem, System};
/// use std::path::Path;
///
/// let system = MockSystem::new()
///     .with_env("HOME", "/home/user")
///     .with_file("/test/file.txt", b"Hello, world!")
///     .with_dir("/test/subdir");
///
/// assert_eq!(system.env_var("HOME").unwrap(), "/home/user");
/// assert!(system.exists(Path::new("/test/file.txt")));
/// ```
#[derive(Clone)]
pub struct MockSystem {
    state: Arc<RwLock<MockSystemState>>,
}

struct MockSystemState {
    env_vars: HashMap<String, String>,
    current_dir: PathBuf,
    files: HashMap<PathBuf, Vec<u8>>,
    dirs: HashSet<PathBuf>,
}

impl MockSystem {
    /// Create a new `MockSystem` with default state
    #[must_use]
    pub fn new() -> Self {
        return Self {
            state: Arc::new(RwLock::new(MockSystemState {
                env_vars: HashMap::new(),
                current_dir: PathBuf::from("/"),
                files: HashMap::new(),
                dirs: HashSet::from([PathBuf::from("/")]),
            })),
        };
    }

    /// Set an environment variable (builder pattern)
    #[must_use]
    pub fn with_env(self, key: &str, value: &str) -> Self {
        let mut state = self.state.write().unwrap();
        state.env_vars.insert(key.to_owned(), value.to_owned());
        drop(state);
        self
    }

    /// Set the current working directory (builder pattern)
    pub fn with_current_dir<P: AsRef<Path>>(self, dir: P) -> Self {
        let mut state = self.state.write().unwrap();
        state.current_dir = dir.as_ref().to_path_buf();
        drop(state);
        self
    }

    /// Add a file with contents (builder pattern)
    pub fn with_file<P: AsRef<Path>>(self, path: P, contents: &[u8]) -> Self {
        let path = path.as_ref().to_path_buf();
        let mut state = self.state.write().unwrap();

        // Ensure parent directories exist
        if let Some(parent) = path.parent() {
            Self::ensure_parent_dirs(&mut state.dirs, parent);
        }

        state.files.insert(path, contents.to_vec());
        drop(state);
        self
    }

    /// Add a directory (builder pattern)
    pub fn with_dir<P: AsRef<Path>>(self, path: P) -> Self {
        let path = path.as_ref().to_path_buf();
        let mut state = self.state.write().unwrap();
        Self::ensure_parent_dirs(&mut state.dirs, &path);
        state.dirs.insert(path);
        drop(state);
        self
    }

    fn ensure_parent_dirs(dirs: &mut HashSet<PathBuf>, path: &Path) {
        let mut ancestors = Vec::new();
        let mut current = path;

        // Collect all ancestors
        while let Some(parent) = current.parent() {
            ancestors.push(parent.to_path_buf());
            current = parent;
            if parent == Path::new("") || parent == Path::new("/") {
                break;
            }
        }

        // Insert all ancestors and the path itself
        for ancestor in ancestors {
            dirs.insert(ancestor);
        }
        dirs.insert(path.to_path_buf());
    }
}

impl Default for MockSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl System for MockSystem {
    fn env_var(&self, key: &str) -> Result<String, VarError> {
        let state = self.state.read().unwrap();
        state.env_vars.get(key).cloned().ok_or(VarError::NotPresent)
    }

    fn current_dir(&self) -> io::Result<PathBuf> {
        let state = self.state.read().unwrap();
        Ok(state.current_dir.clone())
    }

    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        let state = self.state.read().unwrap();
        let bytes = state.files.get(path).ok_or_else(|| {
            return io::Error::new(io::ErrorKind::NotFound, format!("File not found: {path:?}"));
        })?;
        String::from_utf8(bytes.clone()).map_err(|e| {
            return io::Error::new(io::ErrorKind::InvalidData, format!("Invalid UTF-8: {e}"));
        })
    }

    fn write(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        let mut state = self.state.write().unwrap();

        // Ensure parent directories exist
        if let Some(parent) = path.parent()
            && !state.dirs.contains(parent)
        {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Parent directory does not exist: {parent:?}"),
            ));
        }

        state.files.insert(path.to_path_buf(), contents.to_vec());
        Ok(())
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        let mut state = self.state.write().unwrap();
        Self::ensure_parent_dirs(&mut state.dirs, path);
        Ok(())
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        let mut state = self.state.write().unwrap();

        // Remove the directory
        state.dirs.remove(path);

        // Remove all files and subdirectories under this path
        state.files.retain(|p, _| !p.starts_with(path));
        state.dirs.retain(|p| !p.starts_with(path) || p == path);

        Ok(())
    }

    fn copy(&self, from: &Path, to: &Path) -> io::Result<u64> {
        let contents = {
            let state = self.state.read().unwrap();
            state
                .files
                .get(from)
                .ok_or_else(|| {
                    return io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("Source file not found: {from:?}"),
                    );
                })?
                .clone()
        };

        let size = contents.len() as u64;

        // Write to destination
        self.write(to, &contents)?;
        Ok(size)
    }

    fn exists(&self, path: &Path) -> bool {
        let state = self.state.read().unwrap();
        state.files.contains_key(path) || state.dirs.contains(path)
    }

    fn is_file(&self, path: &Path) -> bool {
        let state = self.state.read().unwrap();
        state.files.contains_key(path)
    }

    fn is_dir(&self, path: &Path) -> bool {
        let state = self.state.read().unwrap();
        state.dirs.contains(path)
    }

    fn metadata(&self, path: &Path) -> io::Result<Metadata> {
        // For mock, we need to use a real file's metadata as a template
        // This is a limitation - we'll use a temporary file
        if self.exists(path) {
            // Create a real temporary file to get its metadata
            let temp_dir = tempfile::TempDir::new().map_err(|e| return io::Error::other(e))?;
            let temp_file = temp_dir.path().join("temp");
            std::fs::write(&temp_file, b"")?;
            std::fs::metadata(&temp_file)
        } else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Path not found: {path:?}"),
            ));
        }
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        // For mock, just return absolute path
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            let current = self.current_dir()?;
            Ok(current.join(path))
        }
    }

    fn read_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        let state = self.state.read().unwrap();

        if !state.dirs.contains(path) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Directory not found: {path:?}"),
            ));
        }

        let mut entries = Vec::new();

        // Find all direct children (files and directories)
        for file_path in state.files.keys() {
            if let Some(parent) = file_path.parent()
                && parent == path
            {
                entries.push(file_path.clone());
            }
        }

        for dir_path in &state.dirs {
            if let Some(parent) = dir_path.parent()
                && parent == path
                && dir_path != path
            {
                entries.push(dir_path.clone());
            }
        }

        Ok(entries)
    }

    fn open(&self, path: &Path) -> io::Result<Box<dyn Read + '_>> {
        let state = self.state.read().unwrap();
        let bytes = state.files.get(path).ok_or_else(|| {
            return io::Error::new(io::ErrorKind::NotFound, format!("File not found: {path:?}"));
        })?;
        Ok(Box::new(Cursor::new(bytes.clone())))
    }

    fn create(&self, path: &Path) -> io::Result<Box<dyn Write + '_>> {
        // For create, we need a writer that updates the mock filesystem
        // We'll use a custom writer that captures bytes
        Ok(Box::new(MockWriter {
            path: path.to_path_buf(),
            buffer: Vec::new(),
            system: self.clone(),
        }))
    }
}

/// Custom writer for `MockSystem` that writes to in-memory filesystem
struct MockWriter {
    path: PathBuf,
    buffer: Vec<u8>,
    system: MockSystem,
}

impl Write for MockWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.system.write(&self.path, &self.buffer)?;
        Ok(())
    }
}

impl Drop for MockWriter {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}
