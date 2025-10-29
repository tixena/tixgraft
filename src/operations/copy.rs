//! File and directory copying operations

use crate::error::GraftError;
use crate::system::System;
use crate::utils::fs::create_parent_directories;
use anyhow::{Context as _, Result};
use std::path::{Path, PathBuf};
use tracing::debug;

/// Copy files or directories from source to target
pub fn copy_files(
    system: &dyn System,
    source: &Path,
    target: &str,
    pull_type: &str,
    reset: bool,
) -> Result<usize> {
    let target_path = PathBuf::from(target);

    debug!("Copying files from {source:?} to {target_path:?}");

    // Validate source exists
    if !system.exists(source) {
        debug!("Source path does not exist: {source:?}");
        return Err(GraftError::source(format!(
            "Source path does not exist: {}",
            source.display()
        ))
        .into());
    }

    // Reset target if requested and it's a directory operation
    if reset && pull_type == "directory" && system.exists(&target_path) {
        system
            .remove_dir_all(&target_path)
            .context("Failed to reset target directory")?;
    }

    // Perform the copy based on type
    match pull_type {
        "file" => copy_file(system, source, &target_path),
        "directory" => copy_directory(system, source, &target_path),
        _ => {
            return Err(GraftError::configuration(format!(
                "Invalid pull type: '{pull_type}'. Must be 'file' or 'directory'"
            ))
            .into());
        }
    }
}

/// Copy a single file
pub fn copy_file(system: &dyn System, source: &Path, target: &Path) -> Result<usize> {
    // Validate source is actually a file
    if !system.is_file(source) {
        return Err(
            GraftError::source(format!("Source is not a file: {}", source.display())).into(),
        );
    }

    // Create parent directories for target
    create_parent_directories(system, target)
        .context("Failed to create parent directories for target file")?;

    // Copy the file
    system.copy(source, target).with_context(|| {
        format!(
            "Failed to copy file from {} to {}",
            source.display(),
            target.display()
        )
    })?;

    Ok(1)
}

/// Copy a directory recursively
pub fn copy_directory(system: &dyn System, source: &Path, target: &Path) -> Result<usize> {
    // Validate source is actually a directory
    if !system.is_dir(source) {
        return Err(
            GraftError::source(format!("Source is not a directory: {}", source.display())).into(),
        );
    }

    // Create target directory
    if !system.exists(target) {
        system
            .create_dir_all(target)
            .with_context(|| format!("Failed to create target directory: {}", target.display()))?;
    }

    let mut files_copied = 0;

    // Walk through source directory using System abstraction
    let entries = system
        .walk_dir(source, false, false)
        .with_context(|| format!("Failed to walk directory: {}", source.display()))?;

    for entry in entries {
        let source_path = &entry.path;

        // Calculate relative path from source root
        let relative_path = source_path
            .strip_prefix(source)
            .context("Failed to calculate relative path")?;

        let target_path = target.join(relative_path);

        if entry.is_dir {
            // Create directory
            if !system.exists(&target_path) {
                system.create_dir_all(&target_path).with_context(|| {
                    format!("Failed to create directory: {}", target_path.display())
                })?;
            }
        } else if entry.is_file {
            // Create parent directories if needed
            if let Some(parent) = target_path.parent()
                && !system.exists(parent)
            {
                system.create_dir_all(parent).with_context(|| {
                    format!("Failed to create parent directory: {}", parent.display())
                })?;
            }

            // Copy file
            system.copy(source_path, &target_path).with_context(|| {
                format!(
                    "Failed to copy file from {} to {}",
                    source_path.display(),
                    target_path.display()
                )
            })?;

            files_copied += 1;
        }
    }

    if files_copied == 0 {
        return Err(GraftError::source(format!(
            "No files found to copy in directory: {}",
            source.display()
        ))
        .into());
    }

    Ok(files_copied)
}

/// Calculate the total size of files to be copied (for progress indication)
pub fn calculate_copy_size(system: &dyn System, source: &Path, pull_type: &str) -> Result<u64> {
    match pull_type {
        "file" => {
            if system.is_file(source) {
                Ok(system.metadata(source)?.len())
            } else {
                Ok(0)
            }
        }
        "directory" => {
            let mut total_size = 0;
            if system.is_dir(source) {
                let entries = system.walk_dir(source, false, false)?;
                for entry in entries {
                    if entry.is_file {
                        total_size += system.metadata(&entry.path)?.len();
                    }
                }
            }
            Ok(total_size)
        }
        _ => Ok(0),
    }
}

/// Count files that will be copied
pub fn count_files_to_copy(system: &dyn System, source: &Path, pull_type: &str) -> Result<usize> {
    match pull_type {
        "file" => {
            if system.is_file(source) {
                Ok(1)
            } else {
                Ok(0)
            }
        }
        "directory" => {
            let mut file_count = 0;
            if system.is_dir(source) {
                let entries = system.walk_dir(source, false, false)?;
                for entry in entries {
                    if entry.is_file {
                        file_count += 1;
                    }
                }
            }
            Ok(file_count)
        }
        _ => Ok(0),
    }
}
