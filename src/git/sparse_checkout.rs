//! Git sparse checkout implementation

use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{Result, Context};
use tempfile::TempDir;
use crate::error::GraftError;
use crate::git::Repository;

/// Performs sparse checkout of a specific path from a Git repository
pub struct SparseCheckout {
    pub repository: Repository,
    pub reference: String,
    pub source_path: String,
    pub temp_dir: TempDir,
}

impl SparseCheckout {
    /// Create a new sparse checkout operation
    pub fn new(repository: Repository, reference: String, source_path: String) -> Result<Self> {
        let temp_dir = TempDir::new()
            .context("Failed to create temporary directory for Git operations")?;

        Ok(SparseCheckout {
            repository,
            reference,
            source_path,
            temp_dir,
        })
    }

    /// Execute the sparse checkout operation
    pub fn execute(&self) -> Result<PathBuf> {
        let repo_path = self.temp_dir.path();
        
        // Step 1: Clone with filter and no checkout
        self.clone_repository(repo_path)?;
        
        // Step 2: Initialize sparse checkout
        self.init_sparse_checkout(repo_path)?;
        
        // Step 3: Set sparse checkout patterns
        self.set_sparse_patterns(repo_path)?;
        
        // Step 4: Checkout the specified reference
        self.checkout_reference(repo_path)?;
        
        // Return the path to the checked out source
        Ok(repo_path.join(&self.source_path))
    }

    /// Clone the repository with blob filter and no checkout
    fn clone_repository(&self, repo_path: &Path) -> Result<()> {
        let output = Command::new("git")
            .args([
                "clone",
                "--filter=blob:none",
                "--no-checkout",
                self.repository.git_url(),
                repo_path.to_str().unwrap(),
            ])
            .output()
            .context("Failed to execute git clone command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GraftError::git(format!(
                "Failed to clone repository '{}': {}",
                self.repository.original_url(),
                stderr.trim()
            )).into());
        }

        Ok(())
    }

    /// Initialize sparse checkout configuration
    fn init_sparse_checkout(&self, repo_path: &Path) -> Result<()> {
        let output = Command::new("git")
            .args(["sparse-checkout", "init", "--cone"])
            .current_dir(repo_path)
            .output()
            .context("Failed to execute git sparse-checkout init")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GraftError::git(format!(
                "Failed to initialize sparse checkout: {}",
                stderr.trim()
            )).into());
        }

        Ok(())
    }

    /// Set sparse checkout patterns
    fn set_sparse_patterns(&self, repo_path: &Path) -> Result<()> {
        let output = Command::new("git")
            .args(["sparse-checkout", "set", &self.source_path])
            .current_dir(repo_path)
            .output()
            .context("Failed to execute git sparse-checkout set")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GraftError::git(format!(
                "Failed to set sparse checkout patterns: {}",
                stderr.trim()
            )).into());
        }

        Ok(())
    }

    /// Checkout the specified reference
    fn checkout_reference(&self, repo_path: &Path) -> Result<()> {
        let output = Command::new("git")
            .args(["checkout", &self.reference])
            .current_dir(repo_path)
            .output()
            .context("Failed to execute git checkout")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GraftError::git(format!(
                "Failed to checkout reference '{}': {}",
                self.reference,
                stderr.trim()
            )).into());
        }

        Ok(())
    }

    /// Get the path to the temporary directory
    pub fn temp_path(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Check if the source path exists after checkout
    pub fn source_exists(&self) -> bool {
        self.temp_dir.path().join(&self.source_path).exists()
    }
}

/// Check if Git is available and meets minimum version requirements
pub fn check_git_availability() -> Result<()> {
    let output = Command::new("git")
        .args(["--version"])
        .output()
        .context("Git command not found. Please ensure Git is installed and available in PATH")?;

    if !output.status.success() {
        return Err(GraftError::git(
            "Git command failed to execute properly".to_string()
        ).into());
    }

    let version_output = String::from_utf8_lossy(&output.stdout);
    
    // Extract version number and check if it meets requirements
    // Git sparse-checkout --cone requires Git 2.25+
    if let Some(version_part) = version_output.split_whitespace().nth(2) {
        if let Ok(version) = parse_git_version(version_part) {
            if version < (2, 25, 0) {
                return Err(GraftError::git(format!(
                    "Git version {} is too old. TixGraft requires Git 2.25.0 or later for sparse checkout support",
                    version_part
                )).into());
            }
        }
    }

    Ok(())
}

/// Parse Git version string into tuple (major, minor, patch)
fn parse_git_version(version: &str) -> Result<(u32, u32, u32)> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() >= 3 {
        let major = parts[0].parse().context("Invalid major version")?;
        let minor = parts[1].parse().context("Invalid minor version")?;
        let patch = parts[2].parse().context("Invalid patch version")?;
        Ok((major, minor, patch))
    } else {
        Err(anyhow::anyhow!("Invalid version format"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_git_version() {
        assert_eq!(parse_git_version("2.34.1").unwrap(), (2, 34, 1));
        assert_eq!(parse_git_version("2.25.0").unwrap(), (2, 25, 0));
        assert!(parse_git_version("invalid").is_err());
    }
}
