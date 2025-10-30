//! Git sparse checkout implementation

use crate::error::GraftError;
use crate::git::Repository;
use anyhow::{Context as _, Result};
use core::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;
use tracing::debug;

/// Performs sparse checkout of a specific path from a Git repository
#[non_exhaustive]
pub struct SparseCheckout {
    pub repository: Repository,
    pub reference: String,
    pub source_path: String,
    pub temp_dir: TempDir,
}

impl SparseCheckout {
    /// Create a new sparse checkout operation
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The temporary directory cannot be created
    #[inline]
    pub fn new(repository: Repository, reference: String, source_path: String) -> Result<Self> {
        let temp_dir =
            TempDir::new().context("Failed to create temporary directory for Git operations")?;

        Ok(Self {
            repository,
            reference,
            source_path,
            temp_dir,
        })
    }

    /// Execute the sparse checkout operation
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The repository cannot be cloned
    /// - The sparse checkout cannot be initialized
    /// - The sparse checkout patterns cannot be set
    /// - The reference cannot be checked out
    #[inline]
    pub fn execute(&self) -> Result<PathBuf> {
        let repo_path = self.temp_dir.path();

        debug!("Executing sparse checkout operation: {repo_path:?}");
        // Step 1: Clone with filter and no checkout
        self.clone_repository(repo_path)?;

        debug!("Repository cloned");
        debug!("Initializing sparse checkout");

        // Step 2: Initialize sparse checkout
        Self::init_sparse_checkout(repo_path)?;

        debug!("Sparse checkout initialized");
        debug!("Setting sparse checkout patterns");

        // Step 3: Set sparse checkout patterns
        self.set_sparse_patterns(repo_path)?;

        debug!("Sparse checkout patterns set");
        debug!("Checking out reference");

        // Step 4: Checkout the specified reference
        self.checkout_reference(repo_path)?;

        debug!("Reference checked out");
        let result_path = repo_path.join(&self.source_path);
        debug!("Returning path to checked out source: {result_path:?}");

        // Return the path to the checked out source
        Ok(result_path)
    }

    /// Clone the repository with blob filter and no checkout
    fn clone_repository(&self, repo_path: &Path) -> Result<()> {
        let output = Command::new("git")
            .args([
                "clone",
                "--filter=blob:none",
                "--no-checkout",
                self.repository.git_url()?,
                repo_path.to_str().ok_or_else(|| {
                    anyhow::anyhow!("Failed to convert repository path to string")
                })?,
            ])
            .output()
            .context("Failed to execute git clone command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GraftError::git(format!(
                "Failed to clone repository '{}': {}",
                self.repository.original_url(),
                stderr.trim()
            ))
            .into());
        }

        Ok(())
    }

    /// Initialize sparse checkout configuration
    fn init_sparse_checkout(repo_path: &Path) -> Result<()> {
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
            ))
            .into());
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
            ))
            .into());
        }

        Ok(())
    }

    /// Checkout the specified reference
    fn checkout_reference(&self, repo_path: &Path) -> Result<()> {
        debug!(
            "checkout_reference -> Checking out reference: {}",
            self.reference
        );
        let output = Command::new("git")
            .args(["checkout", &self.reference])
            .current_dir(repo_path)
            .output()
            .context("Failed to execute git checkout")?;

        if !output.status.success() {
            debug!("checkout_reference -> Failed to checkout reference");
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GraftError::git(format!(
                "Failed to checkout reference '{}': {}",
                self.reference,
                stderr.trim()
            ))
            .into());
        }

        debug!("checkout_reference -> Reference checked out successfully");

        Ok(())
    }

    /// Get the path to the temporary directory
    #[must_use]
    #[inline]
    pub fn temp_path(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Check if the source path exists after checkout
    #[must_use]
    #[inline]
    pub fn source_exists(&self) -> bool {
        self.temp_dir.path().join(&self.source_path).exists()
    }

    /// Get diagnostic information about what was actually checked out
    /// This is useful for debugging when `source_exists()` returns false
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The directory cannot be read
    #[inline]
    pub fn get_checkout_diagnostics(&self) -> Result<String> {
        use std::fs;
        let repo_path = self.temp_dir.path();
        let mut diagnostics = String::new();

        writeln!(
            diagnostics,
            "Sparse checkout diagnostics:\n  Repository: {}\n  Reference: {}\n  Requested path: {}\n",
            self.repository.original_url(),
            self.reference,
            self.source_path
        )?;

        writeln!(diagnostics, "  Temp directory: {}", repo_path.display())?;

        // List what was actually checked out
        diagnostics.push_str("  Checked out files:\n");
        if let Ok(entries) = fs::read_dir(repo_path) {
            let mut found_items = Vec::new();
            for entry in entries.flatten() {
                if let Ok(file_name) = entry.file_name().into_string() {
                    // Skip .git directory
                    if file_name != ".git" {
                        found_items.push(file_name);
                    }
                }
            }

            if found_items.is_empty() {
                writeln!(diagnostics, "    (empty - no files were checked out)")?;
            } else {
                for item in found_items {
                    writeln!(diagnostics, "    - {item}")?;
                }
            }
        } else {
            writeln!(diagnostics, "    (unable to read directory)")?;
        }

        // Check sparse-checkout configuration
        let sparse_config = repo_path.join(".git/info/sparse-checkout");
        if sparse_config.exists()
            && let Ok(config_content) = fs::read_to_string(&sparse_config)
        {
            diagnostics.push_str("  Sparse-checkout patterns:\n");
            for line in config_content.lines() {
                writeln!(diagnostics, "    {line}")?;
            }
        }

        Ok(diagnostics)
    }
}

/// Check if Git is available and meets minimum version requirements
///
/// # Errors
///
/// Returns an error if:
/// - The Git command is not found
/// - The Git command failed to execute properly
/// - The Git version is too old
#[inline]
pub fn check_git_availability() -> Result<()> {
    let output = Command::new("git")
        .args(["--version"])
        .output()
        .context("Git command not found. Please ensure Git is installed and available in PATH")?;

    if !output.status.success() {
        return Err(GraftError::git("Git command failed to execute properly".to_owned()).into());
    }

    let version_output = String::from_utf8_lossy(&output.stdout);

    // Extract version number and check if it meets requirements
    // Git sparse-checkout --cone requires Git 2.25+
    if let Some(version_part) = version_output.split_whitespace().nth(2)
        && let Ok(version) = parse_git_version(version_part)
        && version < (2, 25, 0)
    {
        return Err(GraftError::git(format!(
                    "Git version {version_part} is too old. TixGraft requires Git 2.25.0 or later for sparse checkout support"
                )).into());
    }

    Ok(())
}

/// Parse Git version string into tuple (major, minor, patch)
///
/// # Errors
///
/// Returns an error if:
/// - The version string is invalid
#[inline]
pub fn parse_git_version(version: &str) -> Result<(u32, u32, u32)> {
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
