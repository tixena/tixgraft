//! File system utilities

use std::fs;
use std::path::Path;
use std::io::{self, Read, Write};
use anyhow::{Result, Context};

/// Create parent directories for a file path if they don't exist
pub fn create_parent_directories(file_path: &Path) -> Result<()> {
    if let Some(parent) = file_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .with_context(|| format!(
                    "Failed to create parent directories for: {}",
                    file_path.display()
                ))?;
        }
    }
    Ok(())
}

/// Check if a file is binary by examining its first few bytes
pub fn is_binary_file(file_path: &Path) -> Result<bool> {
    if !file_path.is_file() {
        return Ok(false);
    }

    // Read first 1KB to check for binary content
    let mut file = fs::File::open(file_path)
        .with_context(|| format!("Failed to open file: {}", file_path.display()))?;
    
    let mut buffer = vec![0; 1024];
    let bytes_read = file.read(&mut buffer)
        .with_context(|| format!("Failed to read from file: {}", file_path.display()))?;
    
    // Check for null bytes (common indicator of binary files)
    for &byte in &buffer[..bytes_read] {
        if byte == 0 {
            return Ok(true);
        }
    }

    // Check for high ratio of non-ASCII characters
    let non_ascii_count = buffer[..bytes_read]
        .iter()
        .filter(|&&b| b > 127)
        .count();
    
    let ratio = non_ascii_count as f64 / bytes_read as f64;
    Ok(ratio > 0.3) // If more than 30% non-ASCII, likely binary
}

/// Get file size in bytes
pub fn get_file_size(file_path: &Path) -> Result<u64> {
    let metadata = fs::metadata(file_path)
        .with_context(|| format!("Failed to get metadata for: {}", file_path.display()))?;
    Ok(metadata.len())
}

/// Check if directory is empty
pub fn is_directory_empty(dir_path: &Path) -> Result<bool> {
    if !dir_path.is_dir() {
        return Ok(false);
    }

    let mut entries = fs::read_dir(dir_path)
        .with_context(|| format!("Failed to read directory: {}", dir_path.display()))?;
    
    Ok(entries.next().is_none())
}

/// Safely remove directory and all its contents
pub fn remove_dir_safe(dir_path: &Path) -> Result<()> {
    if dir_path.exists() && dir_path.is_dir() {
        fs::remove_dir_all(dir_path)
            .with_context(|| format!(
                "Failed to remove directory: {}",
                dir_path.display()
            ))?;
    }
    Ok(())
}

/// Copy file with progress callback
pub fn copy_file_with_progress<F>(
    source: &Path,
    target: &Path,
    progress_callback: F,
) -> Result<u64>
where
    F: Fn(u64, u64),
{
    let source_size = get_file_size(source)?;
    let mut source_file = fs::File::open(source)
        .with_context(|| format!("Failed to open source file: {}", source.display()))?;

    create_parent_directories(target)?;
    let mut target_file = fs::File::create(target)
        .with_context(|| format!("Failed to create target file: {}", target.display()))?;

    let mut buffer = vec![0; 64 * 1024]; // 64KB buffer
    let mut total_copied = 0u64;

    loop {
        let bytes_read = source_file.read(&mut buffer)
            .with_context(|| "Failed to read from source file")?;

        if bytes_read == 0 {
            break;
        }

        target_file.write_all(&buffer[..bytes_read])
            .with_context(|| "Failed to write to target file")?;

        total_copied += bytes_read as u64;
        progress_callback(total_copied, source_size);
    }

    Ok(total_copied)
}

/// Get human-readable file size
pub fn format_file_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: f64 = 1024.0;

    if bytes == 0 {
        return "0 B".to_string();
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
pub fn paths_are_same(path1: &Path, path2: &Path) -> Result<bool> {
    let canonical1 = fs::canonicalize(path1)
        .with_context(|| format!("Failed to canonicalize path: {}", path1.display()))?;
    let canonical2 = fs::canonicalize(path2)
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
pub fn ensure_dir_exists(dir_path: &Path) -> Result<()> {
    if !dir_path.exists() {
        fs::create_dir_all(dir_path)
            .with_context(|| format!(
                "Failed to create directory: {}",
                dir_path.display()
            ))?;
    } else if !dir_path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("Path exists but is not a directory: {}", dir_path.display())
        ).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_is_binary_file() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create text file
        let text_file = temp_dir.path().join("text.txt");
        fs::write(&text_file, "Hello, world!").unwrap();
        
        // Create binary file
        let binary_file = temp_dir.path().join("binary.bin");
        fs::write(&binary_file, &[0, 1, 2, 3, 0xFF, 0xFE]).unwrap();
        
        assert!(!is_binary_file(&text_file).unwrap());
        assert!(is_binary_file(&binary_file).unwrap());
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
        let temp_dir = TempDir::new().unwrap();
        let nested_file = temp_dir.path().join("a/b/c/file.txt");
        
        assert!(create_parent_directories(&nested_file).is_ok());
        assert!(nested_file.parent().unwrap().exists());
    }

    #[test]
    fn test_is_directory_empty() {
        let temp_dir = TempDir::new().unwrap();
        let empty_dir = temp_dir.path().join("empty");
        let non_empty_dir = temp_dir.path().join("non_empty");
        
        fs::create_dir(&empty_dir).unwrap();
        fs::create_dir(&non_empty_dir).unwrap();
        fs::write(non_empty_dir.join("file.txt"), "content").unwrap();
        
        assert!(is_directory_empty(&empty_dir).unwrap());
        assert!(!is_directory_empty(&non_empty_dir).unwrap());
    }
}
