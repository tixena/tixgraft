//! Finds the `.graft.yaml` files under a directory and links each to the one above it.
//!
//! The walk honors `.gitignore` and `.ignore` files, includes hidden files and follows no symlinks.

use anyhow::{Context as _, Result};
use pollster::FutureExt as _;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use os_shim::System;

/// A `.graft.yaml` file found under the search root, with its location and nearest ancestor.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DiscoveredGraft {
    /// Directory levels below the search root, which is depth 0.
    pub depth: usize,

    /// The directory holding the file, where its replacements and post-commands run.
    pub directory: PathBuf,

    /// The graft in the nearest ancestor directory, if any.
    pub parent: Option<Box<Self>>,

    /// Absolute path to the file.
    pub path: PathBuf,
}

impl DiscoveredGraft {
    /// Returns every ancestor graft, closest first, excluding this one.
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

/// Finds every `.graft.yaml` under `target_dir`, sorted by depth so a parent precedes its children.
///
/// # Errors
///
/// Returns an error if `target_dir` is missing, is not a directory or cannot be walked.
#[inline]
pub fn discover_graft_files(
    system: &dyn System,
    target_dir: &Path,
) -> Result<Vec<DiscoveredGraft>> {
    if !system.exists(target_dir).block_on()? {
        return Err(anyhow::anyhow!(
            "Target directory does not exist: {}",
            target_dir.display()
        ));
    }

    if !system.is_dir(target_dir).block_on()? {
        return Err(anyhow::anyhow!(
            "Target path is not a directory: {}",
            target_dir.display()
        ));
    }

    let mut discoveries = Vec::new();
    let relative_target_dir = system
        .canonicalize(target_dir)
        .block_on()
        .with_context(|| {
            format!(
                "Failed to canonicalize target directory: {}",
                target_dir.display()
            )
        })?;

    let entries = system
        .walk_dir(&relative_target_dir, false, true)
        .block_on()
        .with_context(|| {
            format!(
                "Failed to walk directory: {}",
                relative_target_dir.display()
            )
        })?;

    for entry in entries {
        let path = &entry.path;

        if entry.is_file && path.file_name() == Some(OsStr::new(".graft.yaml")) {
            let directory = path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Failed to get parent directory of .graft.yaml"))?
                .to_path_buf();

            let depth = directory
                .strip_prefix(&relative_target_dir)
                .ok()
                .map_or(0, |relative| relative.components().count());

            discoveries.push((path.clone(), directory, depth));
        }
    }

    discoveries.sort_by_key(|&(_, _, depth)| depth);

    let mut grafts = Vec::new();
    for (path, directory, depth) in discoveries {
        let parent = find_parent_graft(&grafts, &directory);

        grafts.push(DiscoveredGraft {
            depth,
            directory,
            parent,
            path,
        });
    }

    Ok(grafts)
}

/// Returns the deepest graft in an ancestor of `directory`; `grafts` must be sorted by depth.
fn find_parent_graft(grafts: &[DiscoveredGraft], directory: &Path) -> Option<Box<DiscoveredGraft>> {
    for graft in grafts.iter().rev() {
        if directory.starts_with(&graft.directory) && directory != graft.directory {
            return Some(Box::new(graft.clone()));
        }
    }
    None
}

/// Deletes every `.graft.yaml` under `target_dir` and returns how many it removed. Call it once all
/// graft processing is done.
///
/// # Errors
///
/// Returns an error if discovery fails or a file cannot be deleted.
#[inline]
pub fn cleanup_graft_files(system: &dyn System, target_dir: &Path) -> Result<usize> {
    let grafts = discover_graft_files(system, target_dir)?;
    let mut deleted_count: usize = 0;

    for graft in grafts {
        if system.exists(&graft.path).block_on()? {
            system
                .remove_file(&graft.path)
                .block_on()
                .with_context(|| {
                    format!(
                        "Failed to delete .graft.yaml file: {}",
                        graft.path.display()
                    )
                })?;
            deleted_count = deleted_count.saturating_add(1);
        }
    }

    Ok(deleted_count)
}
