//! Discovery of .graft.yaml files in target directories
//!
//! Handles recursive discovery of .graft.yaml files and builds a hierarchy
//! to support context inheritance.

use anyhow::{Context as _, Result};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// A discovered .graft.yaml file with its location and hierarchy information
#[derive(Debug, Clone)]
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
/// Returns grafts sorted by depth (root first) for proper processing order
pub fn discover_graft_files(target_dir: &Path) -> Result<Vec<DiscoveredGraft>> {
    if !target_dir.exists() {
        return Err(anyhow::anyhow!(
            "Target directory does not exist: {}",
            target_dir.display()
        ));
    }

    if !target_dir.is_dir() {
        return Err(anyhow::anyhow!(
            "Target path is not a directory: {}",
            target_dir.display()
        ));
    }

    let mut discoveries = Vec::new();
    let target_dir = target_dir.canonicalize().with_context(|| {
        format!(
            "Failed to canonicalize target directory: {}",
            target_dir.display()
        )
    })?;

    // Walk directory tree and find all .graft.yaml files
    for entry in WalkDir::new(&target_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Check if this is a .graft.yaml file
        if path.is_file() && path.file_name() == Some(std::ffi::OsStr::new(".graft.yaml")) {
            let directory = path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Failed to get parent directory of .graft.yaml"))?
                .to_path_buf();

            // Calculate depth relative to target_dir
            let depth = directory
                .strip_prefix(&target_dir)
                .ok()
                .map_or(0, |p| p.components().count());

            discoveries.push((path.to_path_buf(), directory, depth));
        }
    }

    // Sort by depth (root first)
    discoveries.sort_by_key(|(_, _, depth)| *depth);

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
pub fn cleanup_graft_files(target_dir: &Path) -> Result<usize> {
    let grafts = discover_graft_files(target_dir)?;
    let mut deleted_count = 0;

    for graft in grafts {
        if graft.path.exists() {
            std::fs::remove_file(&graft.path).with_context(|| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_graft_file(dir: &Path) -> Result<()> {
        let graft_path = dir.join(".graft.yaml");
        fs::write(&graft_path, "# Test graft file\n")?;
        Ok(())
    }

    #[test]
    fn test_discover_single_graft() {
        let temp_dir = TempDir::new().unwrap();
        create_graft_file(temp_dir.path()).unwrap();

        let grafts = discover_graft_files(temp_dir.path()).unwrap();
        assert_eq!(grafts.len(), 1);
        assert_eq!(grafts[0].depth, 0);
        assert!(grafts[0].parent.is_none());
    }

    #[test]
    fn test_discover_nested_grafts() {
        let temp_dir = TempDir::new().unwrap();

        // Create root graft
        create_graft_file(temp_dir.path()).unwrap();

        // Create nested directory with graft
        let nested = temp_dir.path().join("nested");
        fs::create_dir(&nested).unwrap();
        create_graft_file(&nested).unwrap();

        // Create doubly nested directory with graft
        let doubly_nested = nested.join("deeper");
        fs::create_dir(&doubly_nested).unwrap();
        create_graft_file(&doubly_nested).unwrap();

        let grafts = discover_graft_files(temp_dir.path()).unwrap();
        assert_eq!(grafts.len(), 3);

        // Check depths
        assert_eq!(grafts[0].depth, 0); // root
        assert_eq!(grafts[1].depth, 1); // nested
        assert_eq!(grafts[2].depth, 2); // doubly nested

        // Check parent relationships
        assert!(grafts[0].parent.is_none());
        assert!(grafts[1].parent.is_some());
        assert!(grafts[2].parent.is_some());

        // Check ancestors
        assert_eq!(grafts[0].ancestors().len(), 0);
        assert_eq!(grafts[1].ancestors().len(), 1);
        assert_eq!(grafts[2].ancestors().len(), 2);
    }

    #[test]
    fn test_cleanup_graft_files() {
        let temp_dir = TempDir::new().unwrap();
        create_graft_file(temp_dir.path()).unwrap();

        let nested = temp_dir.path().join("nested");
        fs::create_dir(&nested).unwrap();
        create_graft_file(&nested).unwrap();

        // Verify files exist
        assert!(temp_dir.path().join(".graft.yaml").exists());
        assert!(nested.join(".graft.yaml").exists());

        // Cleanup
        let deleted = cleanup_graft_files(temp_dir.path()).unwrap();
        assert_eq!(deleted, 2);

        // Verify files are gone
        assert!(!temp_dir.path().join(".graft.yaml").exists());
        assert!(!nested.join(".graft.yaml").exists());
    }

    #[test]
    fn test_discover_no_grafts() {
        let temp_dir = TempDir::new().unwrap();
        let grafts = discover_graft_files(temp_dir.path()).unwrap();
        assert_eq!(grafts.len(), 0);
    }

    #[test]
    fn test_discover_nonexistent_directory() {
        let result = discover_graft_files(Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }
}
