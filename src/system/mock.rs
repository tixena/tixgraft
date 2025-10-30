//! Mock system implementation for testing

#![expect(clippy::module_name_repetitions)]
#![expect(
    clippy::std_instead_of_alloc,
    reason = "I couldn't find that trait in the alloc crate"
)]
#![expect(
    clippy::std_instead_of_core,
    reason = "I couldn't find that trait in the core crate"
)]

use tracing::error;

use super::{System, TempDirHandle, WalkEntry};
use std::collections::{HashMap, HashSet};
use std::env::VarError;
use std::fs::{self, Metadata};
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

// Global counter for generating unique temp directory IDs
static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// In-memory implementation of System trait for testing
///
/// `MockSystem` provides an in-memory filesystem and environment,
/// perfect for fast, isolated unit tests without side effects.
///
/// # Example
/// ```
/// use tixgraft::system::{mock::MockSystem, System};
/// use std::path::Path;
///
/// let system = MockSystem::new()
///     .with_env("HOME", "/home/user").unwrap()
///     .with_file("/test/file.txt", b"Hello, world!").unwrap()
///     .with_dir("/test/subdir").unwrap();
///
/// assert_eq!(system.env_var("HOME").unwrap(), "/home/user");
/// assert!(system.exists(Path::new("/test/file.txt")).unwrap());
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
    #[inline]
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(MockSystemState {
                env_vars: HashMap::new(),
                current_dir: PathBuf::from("/"),
                files: HashMap::new(),
                dirs: HashSet::from([PathBuf::from("/")]),
            })),
        }
    }

    /// Set an environment variable (builder pattern)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The environment variable cannot be set
    #[inline]
    pub fn with_env(self, key: &str, value: &str) -> io::Result<Self> {
        let mut state = self
            .state
            .write()
            .map_err(|e| io::Error::other(e.to_string()))?;
        state.env_vars.insert(key.to_owned(), value.to_owned());
        drop(state);
        Ok(self)
    }

    /// Set the current working directory (builder pattern)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The current working directory cannot be set
    #[inline]
    pub fn with_current_dir<P: AsRef<Path>>(self, dir: P) -> io::Result<Self> {
        let mut state = self
            .state
            .write()
            .map_err(|e| io::Error::other(e.to_string()))?;
        state.current_dir = dir.as_ref().to_path_buf();
        drop(state);
        Ok(self)
    }

    /// Add a file with contents (builder pattern)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be created
    #[inline]
    pub fn with_file<P: AsRef<Path>>(self, path: P, contents: &[u8]) -> io::Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let mut state = self
            .state
            .write()
            .map_err(|e| io::Error::other(e.to_string()))?;

        // Ensure parent directories exist
        if let Some(parent) = path_buf.parent() {
            Self::ensure_parent_dirs(&mut state.dirs, parent);
        }

        state.files.insert(path_buf, contents.to_vec());
        drop(state);
        Ok(self)
    }

    /// Add a directory (builder pattern)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The directory cannot be created
    #[inline]
    pub fn with_dir<P: AsRef<Path>>(self, path: P) -> io::Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let mut state = self
            .state
            .write()
            .map_err(|e| io::Error::other(e.to_string()))?;
        Self::ensure_parent_dirs(&mut state.dirs, &path_buf);
        state.dirs.insert(path_buf);
        drop(state);
        Ok(self)
    }

    #[inline]
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
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl System for MockSystem {
    #[inline]
    #[expect(clippy::map_err_ignore, reason = "This is for VarError")]
    fn env_var(&self, key: &str) -> Result<String, VarError> {
        let state = self.state.read().map_err(|_| VarError::NotPresent)?;
        state.env_vars.get(key).cloned().ok_or(VarError::NotPresent)
    }

    #[inline]
    fn current_dir(&self) -> io::Result<PathBuf> {
        let state = self
            .state
            .read()
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(state.current_dir.clone())
    }

    #[inline]
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        let state = self
            .state
            .read()
            .map_err(|e| io::Error::other(e.to_string()))?;
        let bytes = state.files.get(path).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("File not found: {}", path.display()),
            )
        })?;
        let result = bytes.clone();
        drop(state);
        String::from_utf8(result)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Invalid UTF-8: {e}")))
    }

    #[inline]
    fn write(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        let mut state = self
            .state
            .write()
            .map_err(|e| io::Error::other(e.to_string()))?;

        // Ensure parent directories exist
        if let Some(parent) = path.parent()
            && !state.dirs.contains(parent)
        {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Parent directory does not exist: {}", parent.display()),
            ));
        }

        state.files.insert(path.to_path_buf(), contents.to_vec());
        drop(state);
        Ok(())
    }

    #[inline]
    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        let mut state = self
            .state
            .write()
            .map_err(|e| io::Error::other(e.to_string()))?;
        Self::ensure_parent_dirs(&mut state.dirs, path);
        drop(state);
        Ok(())
    }

    #[inline]
    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        let mut state = self
            .state
            .write()
            .map_err(|e| io::Error::other(e.to_string()))?;

        // Remove the directory
        state.dirs.remove(path);

        // Remove all files and subdirectories under this path
        state.files.retain(|p, _| !p.starts_with(path));
        state.dirs.retain(|p| !p.starts_with(path) || p == path);
        drop(state);
        Ok(())
    }

    #[inline]
    fn remove_file(&self, path: &Path) -> io::Result<()> {
        let mut state = self
            .state
            .write()
            .map_err(|e| io::Error::other(e.to_string()))?;

        if !state.files.contains_key(path) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("File not found: {}", path.display()),
            ));
        }

        state.files.remove(path);
        drop(state);
        Ok(())
    }

    #[inline]
    #[expect(clippy::as_conversions, reason = "This is for usize to u64 conversion")]
    fn copy(&self, from: &Path, to: &Path) -> io::Result<u64> {
        let contents = {
            let state = self
                .state
                .read()
                .map_err(|e| io::Error::other(e.to_string()))?;
            state
                .files
                .get(from)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("Source file not found: {}", from.display()),
                    )
                })?
                .clone()
        };

        let size = contents.len() as u64;

        // Write to destination
        self.write(to, &contents)?;
        Ok(size)
    }

    #[inline]
    fn exists(&self, path: &Path) -> io::Result<bool> {
        let state = self
            .state
            .read()
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(state.files.contains_key(path) || state.dirs.contains(path))
    }

    #[inline]
    fn is_file(&self, path: &Path) -> io::Result<bool> {
        let state = self
            .state
            .read()
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(state.files.contains_key(path))
    }

    #[inline]
    fn is_dir(&self, path: &Path) -> io::Result<bool> {
        let state = self
            .state
            .read()
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(state.dirs.contains(path))
    }

    #[inline]
    fn metadata(&self, path: &Path) -> io::Result<Metadata> {
        // For mock, we need to use a real file's metadata as a template
        // This is a limitation - we'll use a temporary file
        if self.exists(path)? {
            // Create a real temporary file to get its metadata
            let temp_dir = tempfile::TempDir::new().map_err(io::Error::other)?;
            let temp_file = temp_dir.path().join("temp");
            fs::write(&temp_file, b"")?;
            fs::metadata(&temp_file)
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Path not found: {}", path.display()),
            ))
        }
    }

    #[inline]
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        // For mock, just return absolute path
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            let current = self.current_dir()?;
            Ok(current.join(path))
        }
    }

    #[inline]
    fn read_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        let state = self
            .state
            .read()
            .map_err(|e| io::Error::other(e.to_string()))?;

        if !state.dirs.contains(path) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Directory not found: {}", path.display()),
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

        drop(state);

        Ok(entries)
    }

    #[inline]
    fn open(&self, path: &Path) -> io::Result<Box<dyn Read + '_>> {
        let state = self
            .state
            .read()
            .map_err(|e| io::Error::other(e.to_string()))?;
        let bytes = state.files.get(path).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("File not found: {}", path.display()),
            )
        })?;
        let result = bytes.clone();
        drop(state);
        Ok(Box::new(Cursor::new(result)))
    }

    #[inline]
    fn create(&self, path: &Path) -> io::Result<Box<dyn Write + '_>> {
        // For create, we need a writer that updates the mock filesystem
        // We'll use a custom writer that captures bytes
        Ok(Box::new(MockWriter {
            path: path.to_path_buf(),
            buffer: Vec::new(),
            system: self.clone(),
        }))
    }

    #[inline]
    fn walk_dir(
        &self,
        path: &Path,
        _follow_links: bool,
        _hidden: bool,
    ) -> io::Result<Vec<WalkEntry>> {
        let state = self
            .state
            .read()
            .map_err(|e| io::Error::other(e.to_string()))?;

        if !state.dirs.contains(path) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Directory not found: {}", path.display()),
            ));
        }

        let mut entries = Vec::new();
        let mut to_visit: Vec<PathBuf> = vec![path.to_path_buf()];
        let mut visited: HashSet<PathBuf> = HashSet::new();

        while let Some(current) = to_visit.pop() {
            if !visited.insert(current.clone()) {
                continue; // Already visited
            }

            // Skip the root directory itself
            if current != path {
                entries.push(WalkEntry {
                    path: current.clone(),
                    is_file: state.files.contains_key(&current),
                    is_dir: state.dirs.contains(&current),
                });
            }

            // Find all direct children of current directory
            for dir in &state.dirs {
                if let Some(parent) = dir.parent()
                    && parent == current
                    && dir != &current
                {
                    to_visit.push(dir.clone());
                }
            }

            for file_path in state.files.keys() {
                if let Some(parent) = file_path.parent()
                    && parent == current
                {
                    entries.push(WalkEntry {
                        path: file_path.clone(),
                        is_file: true,
                        is_dir: false,
                    });
                }
            }
        }

        // Sort entries by path for deterministic output
        entries.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(entries)
    }

    #[inline]
    fn create_temp_dir(&self) -> io::Result<Box<dyn TempDirHandle>> {
        // Generate unique temp directory ID
        let id = TEMP_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_path = PathBuf::from(format!("/tmp/mock_{id}"));

        // Create the directory in the mock filesystem
        self.create_dir_all(&temp_path)?;

        Ok(Box::new(MockTempDir {
            path: temp_path,
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

#[expect(
    clippy::missing_trait_methods,
    reason = "Only implementing what I need"
)]
impl Write for MockWriter {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.system.write(&self.path, &self.buffer)?;
        Ok(())
    }
}

impl Drop for MockWriter {
    #[inline]
    fn drop(&mut self) {
        match self.flush() {
            Ok(()) => (),
            Err(e) => error!("Failed to flush mock writer: {e}"),
        }
    }
}

/// Mock temporary directory handle that cleans up on drop
#[non_exhaustive]
pub struct MockTempDir {
    path: PathBuf,
    system: MockSystem,
}

impl TempDirHandle for MockTempDir {
    #[inline]
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for MockTempDir {
    #[inline]
    fn drop(&mut self) {
        // Remove the temporary directory from the mock filesystem when dropped
        match self.system.remove_dir_all(&self.path) {
            Ok(()) => (),
            Err(e) => error!("Failed to remove temporary directory: {e}"),
        }
    }
}
