//! Discovery of .graft.yaml files in target directories
//!
//! Handles recursive discovery of .graft.yaml files and builds a hierarchy
//! to support context inheritance.
//!
//! ## Behavior
//!
//! ### Gitignore Support
//! Discovery respects `.gitignore`, `.ignore`, and other ignore files in the directory tree.
//! Files and directories that are ignored will not be searched for `.graft.yaml` files.
//! This behavior is inherited from the `ignore` crate and matches tools like `ripgrep`.
//!
//! ### Symlinks
//! Symbolic links are **not followed** during discovery. Only regular files and directories
//! are traversed. This prevents infinite loops and ensures predictable behavior.
//!
//! ### Hidden Files
//! Hidden files (those starting with `.`) are included in the search, allowing discovery
//! of `.graft.yaml` files themselves.

use anyhow::{Context as _, Result};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use crate::system::System;

/// A discovered .graft.yaml file with its location and hierarchy information
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DiscoveredGraft {
    /// Absolute path to the .graft.yaml file
    pub path: PathBuf,

    /// Directory containing the .graft.yaml file (parent of the file)
    pub directory: PathBuf,

    /// Depth in the directory hierarchy (0 = root)
    pub depth: usize,

    /// Parent graft (if any) for context inheritance
    pub parent: Option<Box<Self>>,
}

impl DiscoveredGraft {
    /// Get all ancestors in order (closest parent first)
    #[must_use]
    #[inline]
    pub fn ancestors(&self) -> Vec<&Self> {
        let mut ancestors = Vec::new();
        let mut current = self.parent.as_deref();
        while let Some(parent) = current {
            ancestors.push(parent);
            current = parent.parent.as_deref();
        }
        ancestors
    }
}

/// Discover all .graft.yaml files in a target directory recursively
///
/// Returns grafts sorted by depth (root first) for proper processing order.
///
/// # Behavior
///
/// - **Gitignore**: Respects `.gitignore`, `.ignore`, and other ignore files
/// - **Symlinks**: Does not follow symbolic links (prevents loops)
/// - **Hidden files**: Includes hidden files (needed to find `.graft.yaml`)
///
/// # Arguments
///
/// * `target_dir` - Directory to search recursively
///
/// # Returns
///
/// Vector of discovered `.graft.yaml` files sorted by depth (0 = root).
/// Each entry includes its path, parent directory, depth, and parent graft reference.
///
/// # Errors
///
/// Returns an error if:
/// - Target directory does not exist
/// - Target path is not a directory
/// - Directory cannot be canonicalized
#[inline]
pub fn discover_graft_files(
    system: &dyn System,
    target_dir: &Path,
) -> Result<Vec<DiscoveredGraft>> {
    if !system.exists(target_dir)? {
        return Err(anyhow::anyhow!(
            "Target directory does not exist: {}",
            target_dir.display()
        ));
    }

    if !system.is_dir(target_dir)? {
        return Err(anyhow::anyhow!(
            "Target path is not a directory: {}",
            target_dir.display()
        ));
    }

    let mut discoveries = Vec::new();
    let relative_target_dir = system.canonicalize(target_dir).with_context(|| {
        format!(
            "Failed to canonicalize target directory: {}",
            target_dir.display()
        )
    })?;

    // Walk directory tree and find all .graft.yaml files using System abstraction
    let entries = system
        .walk_dir(&relative_target_dir, false, false)
        .with_context(|| {
            format!(
                "Failed to walk directory: {}",
                relative_target_dir.display()
            )
        })?;

    for entry in entries {
        let path = &entry.path;

        // Check if this is a .graft.yaml file
        if entry.is_file && path.file_name() == Some(OsStr::new(".graft.yaml")) {
            let directory = path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Failed to get parent directory of .graft.yaml"))?
                .to_path_buf();

            // Calculate depth relative to target_dir
            let depth = directory
                .strip_prefix(&relative_target_dir)
                .ok()
                .map_or(0, |p| p.components().count());

            discoveries.push((path.clone(), directory, depth));
        }
    }

    // Sort by depth (root first)
    discoveries.sort_by_key(|&(_, _, depth)| depth);

    // Build hierarchy with parent relationships
    let mut grafts = Vec::new();
    for (path, directory, depth) in discoveries {
        // Find parent graft (if any)
        let parent = find_parent_graft(&grafts, &directory);

        grafts.push(DiscoveredGraft {
            path,
            directory,
            depth,
            parent,
        });
    }

    Ok(grafts)
}

/// Find the parent graft for a given directory
///
/// The parent is the graft whose directory is an ancestor of the given directory
fn find_parent_graft(grafts: &[DiscoveredGraft], directory: &Path) -> Option<Box<DiscoveredGraft>> {
    // Look for the closest ancestor directory that has a .graft.yaml
    for graft in grafts.iter().rev() {
        if directory.starts_with(&graft.directory) && directory != graft.directory {
            return Some(Box::new(graft.clone()));
        }
    }
    None
}

/// Delete all .graft.yaml files from a target directory
///
/// This should be called after all graft processing is complete
///
/// # Errors
///
/// Returns an error if:
/// - The target directory does not exist
/// - The .graft.yaml files cannot be deleted
#[inline]
pub fn cleanup_graft_files(system: &dyn System, target_dir: &Path) -> Result<usize> {
    let grafts = discover_graft_files(system, target_dir)?;
    let mut deleted_count = 0;

    for graft in grafts {
        if system.exists(&graft.path)? {
            system.remove_file(&graft.path).with_context(|| {
                format!(
                    "Failed to delete .graft.yaml file: {}",
                    graft.path.display()
                )
            })?;
            deleted_count += 1;
        }
    }

    Ok(deleted_count)
}
