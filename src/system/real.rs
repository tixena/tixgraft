//! Real system implementation using `std::env` and `std::fs`

use super::{System, TempDirHandle, WalkEntry};
use ignore::WalkBuilder;
use std::env;
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
#[non_exhaustive]
pub struct RealSystem;

impl RealSystem {
    /// Create a new `RealSystem` instance
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for RealSystem {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl System for RealSystem {
    #[inline]
    fn env_var(&self, key: &str) -> Result<String, VarError> {
        env::var(key)
    }

    #[inline]
    fn current_dir(&self) -> io::Result<PathBuf> {
        env::current_dir()
    }

    #[inline]
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }

    #[inline]
    fn write(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        fs::write(path, contents)
    }

    #[inline]
    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path)
    }

    #[inline]
    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        fs::remove_dir_all(path)
    }

    #[inline]
    fn remove_file(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    #[inline]
    fn copy(&self, from: &Path, to: &Path) -> io::Result<u64> {
        fs::copy(from, to)
    }

    #[inline]
    fn exists(&self, path: &Path) -> io::Result<bool> {
        Ok(path.exists())
    }

    #[inline]
    fn is_file(&self, path: &Path) -> io::Result<bool> {
        Ok(path.is_file())
    }

    #[inline]
    fn is_dir(&self, path: &Path) -> io::Result<bool> {
        Ok(path.is_dir())
    }

    #[inline]
    fn metadata(&self, path: &Path) -> io::Result<Metadata> {
        fs::metadata(path)
    }

    #[inline]
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        fs::canonicalize(path)
    }

    #[inline]
    fn read_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        fs::read_dir(path)?
            .map(|entry| entry.map(|e| e.path()))
            .collect()
    }

    #[inline]
    fn open(&self, path: &Path) -> io::Result<Box<dyn Read + '_>> {
        let file = fs::File::open(path)?;
        Ok(Box::new(file))
    }

    #[inline]
    fn create(&self, path: &Path) -> io::Result<Box<dyn Write + '_>> {
        let file = fs::File::create(path)?;
        Ok(Box::new(file))
    }

    #[inline]
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
            let entry = result.map_err(io::Error::other)?;
            let entry_path = entry.path().to_path_buf();

            entries.push(WalkEntry {
                path: entry_path.clone(),
                is_file: entry_path.is_file(),
                is_dir: entry_path.is_dir(),
            });
        }

        Ok(entries)
    }

    #[inline]
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
    #[inline]
    fn path(&self) -> &Path {
        self.inner.path()
    }
}
