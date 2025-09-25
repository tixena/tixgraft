//! File and directory copying operations

use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Result, Context};
use walkdir::WalkDir;
use crate::error::GraftError;
use crate::utils::fs::create_parent_directories;

/// Copy files or directories from source to target
pub fn copy_files(source: &Path, target: &str, pull_type: &str, reset: bool) -> Result<usize> {
    let target_path = PathBuf::from(target);

    // Validate source exists
    if !source.exists() {
        return Err(GraftError::source(format!(
            "Source path does not exist: {}",
            source.display()
        )).into());
    }

    // Reset target if requested and it's a directory operation
    if reset && pull_type == "directory" && target_path.exists() {
        fs::remove_dir_all(&target_path)
            .context("Failed to reset target directory")?;
    }

    // Perform the copy based on type
    match pull_type {
        "file" => copy_file(source, &target_path),
        "directory" => copy_directory(source, &target_path),
        _ => Err(GraftError::configuration(format!(
            "Invalid pull type: '{}'. Must be 'file' or 'directory'",
            pull_type
        )).into()),
    }
}

/// Copy a single file
fn copy_file(source: &Path, target: &Path) -> Result<usize> {
    // Validate source is actually a file
    if !source.is_file() {
        return Err(GraftError::source(format!(
            "Source is not a file: {}",
            source.display()
        )).into());
    }

    // Create parent directories for target
    create_parent_directories(target)
        .context("Failed to create parent directories for target file")?;

    // Copy the file
    fs::copy(source, target)
        .with_context(|| format!(
            "Failed to copy file from {} to {}",
            source.display(),
            target.display()
        ))?;

    Ok(1)
}

/// Copy a directory recursively
fn copy_directory(source: &Path, target: &Path) -> Result<usize> {
    // Validate source is actually a directory
    if !source.is_dir() {
        return Err(GraftError::source(format!(
            "Source is not a directory: {}",
            source.display()
        )).into());
    }

    // Create target directory
    if !target.exists() {
        fs::create_dir_all(target)
            .with_context(|| format!(
                "Failed to create target directory: {}",
                target.display()
            ))?;
    }

    let mut files_copied = 0;

    // Walk through source directory
    for entry in WalkDir::new(source).min_depth(1) {
        let entry = entry.context("Failed to read directory entry")?;
        let source_path = entry.path();
        
        // Calculate relative path from source root
        let relative_path = source_path.strip_prefix(source)
            .context("Failed to calculate relative path")?;
        
        let target_path = target.join(relative_path);

        if source_path.is_dir() {
            // Create directory
            if !target_path.exists() {
                fs::create_dir_all(&target_path)
                    .with_context(|| format!(
                        "Failed to create directory: {}",
                        target_path.display()
                    ))?;
            }
        } else if source_path.is_file() {
            // Create parent directories if needed
            if let Some(parent) = target_path.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!(
                            "Failed to create parent directory: {}",
                            parent.display()
                        ))?;
                }
            }

            // Copy file
            fs::copy(source_path, &target_path)
                .with_context(|| format!(
                    "Failed to copy file from {} to {}",
                    source_path.display(),
                    target_path.display()
                ))?;

            files_copied += 1;
        }
    }

    if files_copied == 0 {
        return Err(GraftError::source(format!(
            "No files found to copy in directory: {}",
            source.display()
        )).into());
    }

    Ok(files_copied)
}

/// Calculate the total size of files to be copied (for progress indication)
pub fn calculate_copy_size(source: &Path, pull_type: &str) -> Result<u64> {
    match pull_type {
        "file" => {
            if source.is_file() {
                Ok(fs::metadata(source)?.len())
            } else {
                Ok(0)
            }
        }
        "directory" => {
            let mut total_size = 0;
            if source.is_dir() {
                for entry in WalkDir::new(source).min_depth(1) {
                    let entry = entry?;
                    if entry.file_type().is_file() {
                        total_size += fs::metadata(entry.path())?.len();
                    }
                }
            }
            Ok(total_size)
        }
        _ => Ok(0),
    }
}

/// Count files that will be copied
pub fn count_files_to_copy(source: &Path, pull_type: &str) -> Result<usize> {
    match pull_type {
        "file" => {
            if source.is_file() {
                Ok(1)
            } else {
                Ok(0)
            }
        }
        "directory" => {
            let mut file_count = 0;
            if source.is_dir() {
                for entry in WalkDir::new(source).min_depth(1) {
                    let entry = entry?;
                    if entry.file_type().is_file() {
                        file_count += 1;
                    }
                }
            }
            Ok(file_count)
        }
        _ => Ok(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_copy_file() {
        let temp_dir = TempDir::new().unwrap();
        let source_path = temp_dir.path().join("source.txt");
        let target_path = temp_dir.path().join("target.txt");

        // Create source file
        let mut source_file = File::create(&source_path).unwrap();
        writeln!(source_file, "test content").unwrap();

        // Copy file
        let result = copy_file(&source_path, &target_path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);

        // Verify target exists and has correct content
        assert!(target_path.exists());
        let content = fs::read_to_string(&target_path).unwrap();
        assert_eq!(content.trim(), "test content");
    }

    #[test]
    fn test_copy_directory() {
        let temp_dir = TempDir::new().unwrap();
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");

        // Create source directory with files
        fs::create_dir_all(&source_dir).unwrap();
        let file1 = source_dir.join("file1.txt");
        let subdir = source_dir.join("subdir");
        fs::create_dir_all(&subdir).unwrap();
        let file2 = subdir.join("file2.txt");

        let mut f1 = File::create(&file1).unwrap();
        writeln!(f1, "content1").unwrap();

        let mut f2 = File::create(&file2).unwrap();
        writeln!(f2, "content2").unwrap();

        // Copy directory
        let result = copy_directory(&source_dir, &target_dir);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2);

        // Verify target structure
        assert!(target_dir.join("file1.txt").exists());
        assert!(target_dir.join("subdir/file2.txt").exists());
    }
}
