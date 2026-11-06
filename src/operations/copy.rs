//! Copies a pulled file or directory to its target through the `System` abstraction.
//!
//! Also measures what a copy would cover, in bytes and in files.

use crate::error::GraftError;
use crate::utils::fs::create_parent_directories;
use anyhow::{Context as _, Result};
use os_shim::System;
use pollster::FutureExt as _;
use std::path::{Path, PathBuf};
use tracing::debug;

/// Copies `source` to `target` as `pull_type` says and returns the file count. With `reset`, an
/// existing directory target is removed first.
///
/// # Errors
///
/// Returns an error if the source is missing, `pull_type` is unknown or the reset or copy fails.
#[inline]
pub fn copy_files(
    system: &dyn System,
    source: &Path,
    target: &str,
    pull_type: &str,
    reset: bool,
) -> Result<usize> {
    let target_path = PathBuf::from(target);

    debug!("Copying files from {source:?} to {target_path:?}");

    if !system.exists(source).block_on()? {
        debug!("Source path does not exist: {source:?}");
        return Err(GraftError::from_source(format!(
            "Source path does not exist: {}",
            source.display()
        ))
        .into());
    }

    if reset && pull_type == "directory" && system.exists(&target_path).block_on()? {
        system
            .remove_dir_all(&target_path)
            .block_on()
            .context("Failed to reset target directory")?;
    }

    match pull_type {
        "file" => copy_file(system, source, &target_path),
        "directory" => copy_directory(system, source, &target_path),
        _ => Err(GraftError::configuration(format!(
            "Invalid pull type: '{pull_type}'. Must be 'file' or 'directory'"
        ))
        .into()),
    }
}

/// Copies one file, creating the target's parent directories, and returns 1.
///
/// # Errors
///
/// Returns an error if `source` is not a file, or creating the parents or copying fails.
#[inline]
pub fn copy_file(system: &dyn System, source: &Path, target: &Path) -> Result<usize> {
    if !system.is_file(source).block_on()? {
        return Err(
            GraftError::from_source(format!("Source is not a file: {}", source.display())).into(),
        );
    }

    create_parent_directories(system, target)
        .context("Failed to create parent directories for target file")?;

    system.copy(source, target).block_on().with_context(|| {
        format!(
            "Failed to copy file from {} to {}",
            source.display(),
            target.display()
        )
    })?;

    Ok(1)
}

/// Copies the tree under `source` into `target` and returns how many files it copied.
///
/// # Errors
///
/// Returns an error if `source` is not a directory, a copy step fails or it holds no files.
#[inline]
pub fn copy_directory(system: &dyn System, source: &Path, target: &Path) -> Result<usize> {
    if !system.is_dir(source).block_on()? {
        return Err(GraftError::from_source(format!(
            "Source is not a directory: {}",
            source.display()
        ))
        .into());
    }

    if !system.exists(target).block_on()? {
        system
            .create_dir_all(target)
            .block_on()
            .with_context(|| format!("Failed to create target directory: {}", target.display()))?;
    }

    let mut files_copied: usize = 0;

    // walk_dir answers with absolute paths, whatever form `source` takes.
    let walk_root = system
        .current_dir()
        .context("Failed to get current directory")?
        .join(source);

    let entries = system
        .walk_dir(source, false, true)
        .block_on()
        .with_context(|| format!("Failed to walk directory: {}", source.display()))?;

    for entry in entries {
        let source_path = &entry.path;

        let relative_path = source_path
            .strip_prefix(&walk_root)
            .context("Failed to calculate relative path")?;

        let target_path = target.join(relative_path);

        if entry.is_dir {
            if !system.exists(&target_path).block_on()? {
                system
                    .create_dir_all(&target_path)
                    .block_on()
                    .with_context(|| {
                        format!("Failed to create directory: {}", target_path.display())
                    })?;
            }
        } else if entry.is_file {
            if let Some(parent) = target_path.parent()
                && !system.exists(parent).block_on()?
            {
                system.create_dir_all(parent).block_on().with_context(|| {
                    format!("Failed to create parent directory: {}", parent.display())
                })?;
            }

            system
                .copy(source_path, &target_path)
                .block_on()
                .with_context(|| {
                    format!(
                        "Failed to copy file from {} to {}",
                        source_path.display(),
                        target_path.display()
                    )
                })?;

            files_copied = files_copied.saturating_add(1);
        } else {
            debug!("Skipping file: {}", source_path.display());
        }
    }

    if files_copied == 0 {
        return Err(GraftError::from_source(format!(
            "No files found to copy in directory: {}",
            source.display()
        ))
        .into());
    }

    Ok(files_copied)
}

/// Adds up the bytes `copy_files` would copy; a missing source or an unknown type counts as zero.
///
/// # Errors
///
/// Returns an error if the source cannot be inspected or walked.
#[inline]
pub fn calculate_copy_size(system: &dyn System, source: &Path, pull_type: &str) -> Result<u64> {
    match pull_type {
        "file" => {
            if system.is_file(source).block_on()? {
                Ok(system.metadata(source).block_on()?.len)
            } else {
                Ok(0)
            }
        }
        "directory" => {
            let mut total_size: u64 = 0;
            if system.is_dir(source).block_on()? {
                let entries = system.walk_dir(source, false, true).block_on()?;
                for entry in entries {
                    if entry.is_file {
                        total_size =
                            total_size.saturating_add(system.metadata(&entry.path).block_on()?.len);
                    }
                }
            }
            Ok(total_size)
        }
        _ => Ok(0),
    }
}

/// Counts the files `copy_files` would copy; a missing source or an unknown type counts as zero.
///
/// # Errors
///
/// Returns an error if the source cannot be inspected or walked.
#[inline]
pub fn count_files_to_copy(system: &dyn System, source: &Path, pull_type: &str) -> Result<usize> {
    match pull_type {
        "file" => {
            if system.is_file(source).block_on()? {
                Ok(1)
            } else {
                Ok(0)
            }
        }
        "directory" => {
            let mut file_count: usize = 0;
            if system.is_dir(source).block_on()? {
                let entries = system.walk_dir(source, false, true).block_on()?;
                for entry in entries {
                    if entry.is_file {
                        file_count = file_count.saturating_add(1);
                    }
                }
            }
            Ok(file_count)
        }
        _ => Ok(0),
    }
}
