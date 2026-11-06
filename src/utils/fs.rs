//! File system helpers, most of them built on the `System` abstraction.
//!
//! They cover binary detection, copying, directory handling and size formatting.

use anyhow::{Context as _, Result};
use futures_util::{AsyncReadExt as _, AsyncWriteExt as _};
use os_shim::System;
use os_shim::error::{ErrorKind, StorageError};
use pollster::FutureExt as _;
use std::path::Path;
use tracing::debug;

/// Lowercase extensions treated as text without reading the file.
const TEXT_EXTENSIONS: &[&str] = &[
    "bash",
    "c",
    "cc",
    "cjs",
    "conf",
    "config",
    "cpp",
    "css",
    "cxx",
    "csv",
    "dockerfile",
    "dockerignore",
    "editorconfig",
    "env",
    "fish",
    "gemfile",
    "gitignore",
    "go",
    "graphql",
    "h",
    "htm",
    "html",
    "ini",
    "java",
    "js",
    "json",
    "jsx",
    "kt",
    "less",
    "log",
    "makefile",
    "markdown",
    "md",
    "mjs",
    "php",
    "pl",
    "properties",
    "proto",
    "py",
    "pyi",
    "rakefile",
    "rb",
    "rs",
    "rst",
    "sass",
    "scala",
    "scss",
    "sh",
    "sql",
    "swift",
    "thrift",
    "toml",
    "ts",
    "tsv",
    "tsx",
    "txt",
    "xml",
    "yaml",
    "yml",
    "zsh",
];

/// Create parent directories for a file path if they don't exist.
///
/// # Errors
///
/// Returns an error if the parent directories cannot be created.
#[inline]
pub fn create_parent_directories(system: &dyn System, file_path: &Path) -> Result<()> {
    if let Some(parent) = file_path.parent()
        && !system.exists(parent).block_on()?
    {
        system.create_dir_all(parent).block_on().with_context(|| {
            format!(
                "Failed to create parent directories for: {}",
                file_path.display()
            )
        })?;
    }
    Ok(())
}

/// True when the first 8192 bytes hold a null byte or invalid UTF-8, which includes a multibyte
/// character cut at the limit; a non-file, an empty file and a known text extension are never
/// binary.
///
/// # Errors
///
/// Returns an error if the file cannot be opened or read.
#[inline]
pub fn is_binary_file(system: &dyn System, file_path: &Path) -> Result<bool> {
    if !system.is_file(file_path).block_on()? {
        return Ok(false);
    }

    if let Some(extension) = file_path.extension().and_then(|ext| ext.to_str()) {
        let ext = extension.to_lowercase();
        if TEXT_EXTENSIONS.contains(&ext.as_str()) {
            return Ok(false);
        }
    }

    let mut file = system
        .open(file_path)
        .block_on()
        .with_context(|| format!("Failed to open file: {}", file_path.display()))?;

    let mut buffer = vec![0; 8192];
    let bytes_read = file
        .read(&mut buffer)
        .block_on()
        .with_context(|| format!("Failed to read from file: {}", file_path.display()))?;

    if bytes_read == 0 {
        return Ok(false);
    }

    let read_slice = buffer.get(..bytes_read).unwrap_or(&buffer);
    for &byte in read_slice {
        if byte == 0 {
            return Ok(true);
        }
    }

    if str::from_utf8(read_slice).is_ok() {
        return Ok(false);
    }

    // Text in another encoding cannot be told from binary, so it counts as binary.
    Ok(true)
}

/// Get file size in bytes.
///
/// # Errors
///
/// Returns an error if the file size cannot be retrieved.
#[inline]
pub fn get_file_size(system: &dyn System, file_path: &Path) -> Result<u64> {
    let metadata = system
        .metadata(file_path)
        .block_on()
        .with_context(|| format!("Failed to get metadata for: {}", file_path.display()))?;
    Ok(metadata.len)
}

/// False when `dir_path` is not a directory, as well as when it has entries.
///
/// # Errors
///
/// Returns an error if the directory cannot be read.
#[inline]
pub fn is_directory_empty(system: &dyn System, dir_path: &Path) -> Result<bool> {
    if !system.is_dir(dir_path).block_on()? {
        return Ok(false);
    }

    let entries = system
        .read_dir(dir_path)
        .block_on()
        .with_context(|| format!("Failed to read directory: {}", dir_path.display()))?;

    Ok(entries.is_empty())
}

/// Removes a directory and its contents; a missing path or a non-directory is left alone.
///
/// # Errors
///
/// Returns an error if the directory cannot be removed.
#[inline]
pub fn remove_dir_safe(system: &dyn System, dir_path: &Path) -> Result<()> {
    if system.exists(dir_path).block_on()? && system.is_dir(dir_path).block_on()? {
        system
            .remove_dir_all(dir_path)
            .block_on()
            .with_context(|| format!("Failed to remove directory: {}", dir_path.display()))?;
    }
    Ok(())
}

/// Copies in 64 KB chunks, calling `progress_callback` with the bytes copied and the source size
/// after each, and returns the bytes copied.
///
/// # Errors
///
/// Returns an error if the source cannot be read or the target cannot be written or closed.
#[inline]
#[expect(clippy::as_conversions, reason = "This is usize to u64 conversion")]
pub fn copy_file_with_progress<F>(
    system: &dyn System,
    source: &Path,
    target: &Path,
    progress_callback: F,
) -> Result<u64>
where
    F: Fn(u64, u64),
{
    let source_size = get_file_size(system, source)?;
    let mut source_file = system
        .open(source)
        .block_on()
        .with_context(|| format!("Failed to open source file: {}", source.display()))?;

    create_parent_directories(system, target)?;
    let mut target_file = system
        .create(target)
        .block_on()
        .with_context(|| format!("Failed to create target file: {}", target.display()))?;

    let mut buffer = vec![0; 64 * 1024];
    let mut total_copied: u64 = 0;

    loop {
        let bytes_read = source_file
            .read(&mut buffer)
            .block_on()
            .with_context(|| "Failed to read from source file")?;

        if bytes_read == 0 {
            break;
        }

        let write_slice = buffer.get(..bytes_read).unwrap_or(&buffer);
        target_file
            .write_all(write_slice)
            .block_on()
            .with_context(|| "Failed to write to target file")?;

        total_copied = total_copied.saturating_add(bytes_read as u64);
        progress_callback(total_copied, source_size);
    }

    target_file
        .close()
        .block_on()
        .with_context(|| format!("Failed to close target file: {}", target.display()))?;

    Ok(total_copied)
}

/// Formats with 1024-based units up to TB: whole bytes below 1 KB, one decimal place above.
#[must_use]
#[inline]
#[expect(clippy::as_conversions, reason = "This is for number formatting")]
#[expect(clippy::cast_precision_loss, reason = "This is for number formatting")]
#[expect(
    clippy::float_arithmetic,
    reason = "floating-point division required for human-readable size formatting"
)]
pub fn format_file_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: f64 = 1024.0;

    if bytes == 0 {
        return "0 B".to_owned();
    }

    let mut size = bytes as f64;
    let mut unit_index: usize = 0;
    let max_index = UNITS.len().saturating_sub(1);

    while size >= THRESHOLD && unit_index < max_index {
        size /= THRESHOLD;
        unit_index = unit_index.saturating_add(1);
    }

    let unit = UNITS.get(unit_index).unwrap_or(&"B");
    if unit_index == 0 {
        format!("{bytes} {unit}")
    } else {
        format!("{size:.1} {unit}")
    }
}

/// Compares canonical forms, so symbolic links and relative paths are resolved first.
///
/// # Errors
///
/// Returns an error if either path cannot be canonicalized.
#[inline]
pub fn paths_are_same(system: &dyn System, path1: &Path, path2: &Path) -> Result<bool> {
    let canonical1 = system
        .canonicalize(path1)
        .block_on()
        .with_context(|| format!("Failed to canonicalize path: {}", path1.display()))?;
    let canonical2 = system
        .canonicalize(path2)
        .block_on()
        .with_context(|| format!("Failed to canonicalize path: {}", path2.display()))?;

    Ok(canonical1 == canonical2)
}

/// Creates a directory whose name starts with `prefix`, removed when the returned handle drops.
///
/// # Errors
///
/// Returns an error if the temporary directory cannot be created.
#[inline]
pub fn create_temp_dir(prefix: &str) -> Result<tempfile::TempDir> {
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir()
        .context("Failed to create temporary directory")
}

/// Ensure a directory exists, creating it if necessary.
///
/// # Errors
///
/// Returns an error if the path exists but is not a directory, or the directory cannot be created.
#[inline]
pub fn ensure_dir_exists(system: &dyn System, dir_path: &Path) -> Result<()> {
    if !system.exists(dir_path).block_on()? {
        system
            .create_dir_all(dir_path)
            .block_on()
            .with_context(|| format!("Failed to create directory: {}", dir_path.display()))?;
    } else if !system.is_dir(dir_path).block_on()? {
        return Err(StorageError::new(
            ErrorKind::AlreadyExists,
            format!("Path exists but is not a directory: {}", dir_path.display()),
        )
        .into());
    } else {
        debug!("Directory already exists: {}", dir_path.display());
    }
    Ok(())
}
