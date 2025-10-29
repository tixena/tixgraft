//! Real system implementation using `std::env` and `std::fs`

use super::{System, TempDirHandle, WalkEntry};
use ignore::WalkBuilder;
use std::env::VarError;
use std::fs::{self, Metadata};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Production implementation of System trait
///
/// This implementation directly delegates to the standard library's
/// environment and filesystem functions. It's a zero-cost abstraction
/// that provides no overhead in production.
#[derive(Debug, Clone, Copy)]
pub struct RealSystem;

impl RealSystem {
    /// Create a new `RealSystem` instance
    #[must_use]
    pub const fn new() -> Self {
        return Self;
    }
}

impl Default for RealSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl System for RealSystem {
    fn env_var(&self, key: &str) -> Result<String, VarError> {
        std::env::var(key)
    }

    fn current_dir(&self) -> io::Result<PathBuf> {
        std::env::current_dir()
    }

    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }

    fn write(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        fs::write(path, contents)
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path)
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        fs::remove_dir_all(path)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    fn copy(&self, from: &Path, to: &Path) -> io::Result<u64> {
        fs::copy(from, to)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn metadata(&self, path: &Path) -> io::Result<Metadata> {
        fs::metadata(path)
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        fs::canonicalize(path)
    }

    fn read_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        fs::read_dir(path)?
            .map(|entry| entry.map(|e| e.path()))
            .collect()
    }

    fn open(&self, path: &Path) -> io::Result<Box<dyn Read + '_>> {
        let file = fs::File::open(path)?;
        Ok(Box::new(file))
    }

    fn create(&self, path: &Path) -> io::Result<Box<dyn Write + '_>> {
        let file = fs::File::create(path)?;
        Ok(Box::new(file))
    }

    fn walk_dir(
        &self,
        path: &Path,
        follow_links: bool,
        hidden: bool,
    ) -> io::Result<Vec<WalkEntry>> {
        let mut entries = Vec::new();

        // Use WalkBuilder from ignore crate to respect .gitignore files
        for result in WalkBuilder::new(path)
            .follow_links(follow_links)
            .hidden(hidden)
            .build()
            .skip(1)
        // Skip the root directory itself
        {
            let entry = result.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            let entry_path = entry.path().to_path_buf();

            entries.push(WalkEntry {
                path: entry_path.clone(),
                is_file: entry_path.is_file(),
                is_dir: entry_path.is_dir(),
            });
        }

        Ok(entries)
    }

    fn create_temp_dir(&self) -> io::Result<Box<dyn TempDirHandle>> {
        let temp_dir = TempDir::new()?;
        Ok(Box::new(RealTempDir { inner: temp_dir }))
    }
}

/// Real filesystem temporary directory handle
pub struct RealTempDir {
    inner: TempDir,
}

impl TempDirHandle for RealTempDir {
    fn path(&self) -> &Path {
        self.inner.path()
    }
}
