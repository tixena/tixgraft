//! File system utilities

use crate::system::System;
use anyhow::{Context as _, Result};
use std::io::{self, Read as _, Write as _};
use std::path::Path;

/// Known text file extensions for binary detection
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

/// Create parent directories for a file path if they don't exist
pub fn create_parent_directories(system: &dyn System, file_path: &Path) -> Result<()> {
    if let Some(parent) = file_path.parent()
        && !system.exists(parent)
    {
        system.create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create parent directories for: {}",
                file_path.display()
            )
        })?;
    }
    Ok(())
}

/// Check if a file is binary by examining its extension and content
pub fn is_binary_file(system: &dyn System, file_path: &Path) -> Result<bool> {
    // If it's a directory, it's not a binary file
    if !system.is_file(file_path) {
        return Ok(false);
    }

    // Check if it has a known text file extension
    if let Some(extension) = file_path.extension().and_then(|e| e.to_str()) {
        let ext = extension.to_lowercase();
        if TEXT_EXTENSIONS.contains(&ext.as_str()) {
            return Ok(false); // Known text file extension
        }
    }

    // Fallback: check file content
    let mut file = system
        .open(file_path)
        .with_context(|| format!("Failed to open file: {}", file_path.display()))?;

    let mut buffer = vec![0; 8192];
    let bytes_read = file
        .read(&mut buffer)
        .with_context(|| format!("Failed to read from file: {}", file_path.display()))?;

    if bytes_read == 0 {
        return Ok(false); // Empty file is text
    }

    // Check for null bytes - text files don't have them
    for &byte in &buffer[..bytes_read] {
        if byte == 0 {
            return Ok(true); // Has null byte = binary
        }
    }

    // Check if it's valid UTF-8
    if core::str::from_utf8(&buffer[..bytes_read]).is_ok() {
        return Ok(false); // Valid UTF-8 = text
    }

    // Not valid UTF-8 and no null bytes = assume binary
    Ok(true)
}

/// Get file size in bytes
pub fn get_file_size(system: &dyn System, file_path: &Path) -> Result<u64> {
    let metadata = system
        .metadata(file_path)
        .with_context(|| format!("Failed to get metadata for: {}", file_path.display()))?;
    Ok(metadata.len())
}

/// Check if directory is empty
pub fn is_directory_empty(system: &dyn System, dir_path: &Path) -> Result<bool> {
    if !system.is_dir(dir_path) {
        return Ok(false);
    }

    let entries = system
        .read_dir(dir_path)
        .with_context(|| format!("Failed to read directory: {}", dir_path.display()))?;

    Ok(entries.is_empty())
}

/// Safely remove directory and all its contents
pub fn remove_dir_safe(system: &dyn System, dir_path: &Path) -> Result<()> {
    if system.exists(dir_path) && system.is_dir(dir_path) {
        system
            .remove_dir_all(dir_path)
            .with_context(|| format!("Failed to remove directory: {}", dir_path.display()))?;
    }
    Ok(())
}

/// Copy file with progress callback
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
        .with_context(|| format!("Failed to open source file: {}", source.display()))?;

    create_parent_directories(system, target)?;
    let mut target_file = system
        .create(target)
        .with_context(|| format!("Failed to create target file: {}", target.display()))?;

    let mut buffer = vec![0; 64 * 1024]; // 64KB buffer
    let mut total_copied = 0u64;

    loop {
        let bytes_read = source_file
            .read(&mut buffer)
            .with_context(|| "Failed to read from source file")?;

        if bytes_read == 0 {
            break;
        }

        target_file
            .write_all(&buffer[..bytes_read])
            .with_context(|| "Failed to write to target file")?;

        total_copied += bytes_read as u64;
        progress_callback(total_copied, source_size);
    }

    Ok(total_copied)
}

/// Get human-readable file size
#[must_use]
pub fn format_file_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: f64 = 1024.0;

    if bytes == 0 {
        return "0 B".to_owned();
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= THRESHOLD && unit_index < UNITS.len() - 1 {
        size /= THRESHOLD;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Check if two paths point to the same file/directory
pub fn paths_are_same(system: &dyn System, path1: &Path, path2: &Path) -> Result<bool> {
    let canonical1 = system
        .canonicalize(path1)
        .with_context(|| format!("Failed to canonicalize path: {}", path1.display()))?;
    let canonical2 = system
        .canonicalize(path2)
        .with_context(|| format!("Failed to canonicalize path: {}", path2.display()))?;

    Ok(canonical1 == canonical2)
}

/// Create a temporary directory with a specific prefix
pub fn create_temp_dir(prefix: &str) -> Result<tempfile::TempDir> {
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir()
        .context("Failed to create temporary directory")
}

/// Ensure a directory exists, creating it if necessary
pub fn ensure_dir_exists(system: &dyn System, dir_path: &Path) -> Result<()> {
    if !system.exists(dir_path) {
        system
            .create_dir_all(dir_path)
            .with_context(|| format!("Failed to create directory: {}", dir_path.display()))?;
    } else if !system.is_dir(dir_path) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("Path exists but is not a directory: {}", dir_path.display()),
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::RealSystem;
    use tempfile::TempDir;

    #[test]
    fn test_is_binary_file() {
        let system = RealSystem::new();
        let temp_dir = TempDir::new().unwrap();

        // Create text file
        let text_file = temp_dir.path().join("text.txt");
        system.write(&text_file, b"Hello, world!").unwrap();

        // Create UTF-8 file with emojis and international characters
        let utf8_file = temp_dir.path().join("utf8.txt");
        system
            .write(
                &utf8_file,
                "Hello 🏗️ World! ┌─ UTF-8 文字 العربية".as_bytes(),
            )
            .unwrap();

        // Create binary file with null bytes
        let binary_file = temp_dir.path().join("binary.bin");
        system
            .write(&binary_file, &[0, 1, 2, 3, 0xFF, 0xFE])
            .unwrap();

        assert!(!is_binary_file(&system, &text_file).unwrap());
        assert!(!is_binary_file(&system, &utf8_file).unwrap());
        assert!(is_binary_file(&system, &binary_file).unwrap());
    }

    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(0), "0 B");
        assert_eq!(format_file_size(1023), "1023 B");
        assert_eq!(format_file_size(1024), "1.0 KB");
        assert_eq!(format_file_size(1536), "1.5 KB");
        assert_eq!(format_file_size(1048576), "1.0 MB");
    }

    #[test]
    fn test_create_parent_directories() {
        let system = RealSystem::new();
        let temp_dir = TempDir::new().unwrap();
        let nested_file = temp_dir.path().join("a/b/c/file.txt");

        assert!(create_parent_directories(&system, &nested_file).is_ok());
        assert!(nested_file.parent().unwrap().exists());
    }

    #[test]
    fn test_is_directory_empty() {
        let system = RealSystem::new();
        let temp_dir = TempDir::new().unwrap();
        let empty_dir = temp_dir.path().join("empty");
        let non_empty_dir = temp_dir.path().join("non_empty");

        system.create_dir_all(&empty_dir).unwrap();
        system.create_dir_all(&non_empty_dir).unwrap();
        system
            .write(&non_empty_dir.join("file.txt"), b"content")
            .unwrap();

        assert!(is_directory_empty(&system, &empty_dir).unwrap());
        assert!(!is_directory_empty(&system, &non_empty_dir).unwrap());
    }
}
