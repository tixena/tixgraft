//! Git repository handling and URL parsing

use crate::error::GraftError;
use crate::system::System;
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Represents a repository source - either Git or local filesystem
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum RepositorySource {
    /// Git repository with URL
    Git {
        /// Original URL as provided by user
        original_url: String,
        /// Normalized URL for Git operations
        normalized_url: String,
    },
    /// Local filesystem path
    Local {
        /// Original path string as provided by user
        original_path: String,
        /// Resolved absolute path
        resolved_path: PathBuf,
    },
}

/// Represents a repository with URL normalization
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Repository {
    /// Original URL as provided by user
    pub url: String,
    /// Repository source
    pub source: RepositorySource,
}

impl Repository {
    /// Create a new repository from a URL or local path
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The repository source cannot be detected
    #[inline]
    pub fn new(system: &dyn System, url: &str) -> Result<Self> {
        let source = detect_source_type(system, url)?;

        Ok(Self {
            url: url.to_owned(),
            source,
        })
    }

    /// Get the normalized URL for Git operations (panics if called on Local source)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The repository is a local source
    #[inline]
    pub fn git_url(&self) -> Result<&str> {
        match self.source {
            RepositorySource::Git {
                ref normalized_url, ..
            } => Ok(normalized_url),
            RepositorySource::Local { .. } => {
                Err(GraftError::git("git_url() called on Local repository source").into())
            }
        }
    }

    /// Get the original URL as provided
    #[must_use]
    #[inline]
    pub fn original_url(&self) -> &str {
        &self.url
    }

    /// Check if this is a Git repository
    #[must_use]
    #[inline]
    pub const fn is_git(&self) -> bool {
        matches!(self.source, RepositorySource::Git { .. })
    }

    /// Check if this is a local filesystem source
    #[must_use]
    #[inline]
    pub const fn is_local(&self) -> bool {
        matches!(self.source, RepositorySource::Local { .. })
    }

    /// Get the local path (returns None if this is a Git source)
    #[must_use]
    #[inline]
    pub const fn local_path(&self) -> Option<&PathBuf> {
        match self.source {
            RepositorySource::Local {
                ref resolved_path, ..
            } => Some(resolved_path),
            RepositorySource::Git { .. } => None,
        }
    }
}

/// Detect whether the source is a Git repository or local filesystem path
fn detect_source_type(system: &dyn System, url: &str) -> Result<RepositorySource> {
    // ONLY accept "file:" prefix for local filesystem sources
    // This is explicit and leaves room for future prefixes like s3:, gdrive:, etc.
    if url.starts_with("file:") {
        // Support both file:// and file:/ formats
        let path_str = if url.starts_with("file://") {
            url.strip_prefix("file://")
                .ok_or_else(|| anyhow::anyhow!("Failed to strip prefix from URL"))?
        } else {
            url.strip_prefix("file:")
                .ok_or_else(|| anyhow::anyhow!("Failed to strip prefix from URL"))?
        };
        return create_local_source(system, url, path_str);
    }

    // Everything else is treated as a Git repository
    let normalized_url = normalize_repository_url(url)?;
    Ok(RepositorySource::Git {
        original_url: url.to_owned(),
        normalized_url,
    })
}

/// Create a local repository source, resolving the path
fn create_local_source(
    system: &dyn System,
    original: &str,
    path_str: &str,
) -> Result<RepositorySource> {
    // Expand ~ to home directory
    let expanded_path = if path_str.starts_with('~') {
        let home = system
            .env_var("HOME")
            .or_else(|_| system.env_var("USERPROFILE"))
            .map_err(|err| {
                GraftError::configuration(format!(
                    "Cannot determine home directory for ~ expansion. Error: {err}"
                ))
            })?;
        path_str.replacen('~', &home, 1)
    } else {
        path_str.to_owned()
    };

    let path = PathBuf::from(&expanded_path);

    // Resolve to absolute path
    let resolved_path = if path.is_absolute() {
        path
    } else {
        system
            .current_dir()
            .map_err(|e| GraftError::filesystem(format!("Cannot get current directory: {e}")))?
            .join(&path)
    };

    // Verify the path exists
    if !system.exists(&resolved_path)? {
        return Err(GraftError::from_source(format!(
            "Local repository path does not exist: '{}'",
            resolved_path.display()
        ))
        .into());
    }

    // Verify it's a directory
    if !system.is_dir(&resolved_path)? {
        return Err(GraftError::from_source(format!(
            "Local repository path is not a directory: '{}'",
            resolved_path.display()
        ))
        .into());
    }

    Ok(RepositorySource::Local {
        original_path: original.to_owned(),
        resolved_path,
    })
}

/// Normalize a repository URL to a format suitable for Git operations
fn normalize_repository_url(url: &str) -> Result<String> {
    // Handle different URL formats
    if url.starts_with("https://") || url.starts_with("http://") {
        // Already a full HTTP/HTTPS URL
        if Path::new(url)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("git"))
        {
            Ok(url.to_owned())
        } else {
            Ok(format!("{url}.git"))
        }
    } else if url.starts_with("git@") {
        // SSH URL - use as-is
        Ok(url.to_owned())
    } else if url.contains('/') && !url.contains(':') {
        // Short format: my_organization/repo -> https://github.com/my_organization/repo.git
        if url.matches('/').count() == 1 {
            Ok(format!("https://github.com/{url}.git"))
        } else {
            Err(GraftError::configuration(format!(
                "Invalid repository format: '{url}'. Expected format: 'org/repo'"
            ))
            .into())
        }
    } else {
        Err(GraftError::configuration(format!(
            "Unsupported repository URL format: '{url}'\n\
            Supported formats:\n\
            - Short: my_organization/repo\n\
            - HTTPS: https://github.com/my_organization/repo.git\n\
            - SSH: git@github.com:my_organization/repo.git\n\
            - Local: file:///path/to/repo or ~/path/to/repo"
        ))
        .into())
    }
}

/// Validate that a repository URL is accessible
///
/// # Errors
///
/// Returns an error if:
/// - The repository URL is empty
/// - The Git reference (tag/branch) is empty
/// - The repository is local
#[inline]
pub fn validate_repository_access(repo: &Repository, tag: &str) -> Result<()> {
    // For local repositories, we've already validated the path exists in detect_source_type
    if repo.is_local() {
        // Local sources don't need tag validation (ignored)
        return Ok(());
    }

    // For Git repositories, validate URL and tag
    match repo.source {
        RepositorySource::Git {
            ref normalized_url, ..
        } => {
            if normalized_url.is_empty() {
                return Err(GraftError::git("Repository URL cannot be empty".to_owned()).into());
            }

            if tag.is_empty() {
                return Err(GraftError::git(
                    "Git reference (tag/branch) cannot be empty".to_owned(),
                )
                .into());
            }
        }
        RepositorySource::Local { .. } => {
            // Already handled above
        }
    }

    Ok(())
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "These are unit tests")]
mod tests {
    use super::*;
    use crate::system::real::RealSystem;
    use tempfile::TempDir;

    #[test]
    fn normalize_repository_url_tst() {
        // Short format
        assert_eq!(
            normalize_repository_url("my_organization/repo").unwrap(),
            "https://github.com/my_organization/repo.git"
        );

        // HTTPS without .git
        assert_eq!(
            normalize_repository_url("https://github.com/my_organization/repo").unwrap(),
            "https://github.com/my_organization/repo.git"
        );

        // HTTPS with .git
        assert_eq!(
            normalize_repository_url("https://github.com/my_organization/repo.git").unwrap(),
            "https://github.com/my_organization/repo.git"
        );

        // SSH
        assert_eq!(
            normalize_repository_url("git@github.com:my_organization/repo.git").unwrap(),
            "git@github.com:my_organization/repo.git"
        );
    }

    #[test]
    fn invalid_repository_urls() {
        normalize_repository_url("invalid").unwrap_err();
        normalize_repository_url("").unwrap_err();
        normalize_repository_url("too/many/slashes").unwrap_err();
    }

    #[test]
    fn detect_git_source() {
        let system = RealSystem::new();

        // Short format
        let repo = Repository::new(&system, "my_organization/repo").unwrap();
        assert!(repo.is_git());
        assert!(!repo.is_local());
        assert_eq!(
            repo.git_url().unwrap(),
            "https://github.com/my_organization/repo.git"
        );

        // HTTPS
        let repo_2 =
            Repository::new(&system, "https://github.com/my_organization/repo.git").unwrap();
        assert!(repo_2.is_git());
        assert!(!repo_2.is_local());

        // SSH
        let repo_3 = Repository::new(&system, "git@github.com:my_organization/repo.git").unwrap();
        assert!(repo_3.is_git());
        assert!(!repo_3.is_local());
    }

    #[test]
    fn detect_local_source_with_file_prefix() {
        let system = RealSystem::new();
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        // Test with file:// prefix
        let url = format!("file://{}", path.display());
        let repo = Repository::new(&system, &url).unwrap();

        assert!(repo.is_local());
        assert!(!repo.is_git());
        assert!(repo.local_path().is_some());
        assert_eq!(repo.local_path().unwrap(), path);
    }

    #[test]
    fn detect_local_source_with_absolute_path() {
        let system = RealSystem::new();
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();
        let url = format!("file:{path}");

        let repo = Repository::new(&system, &url).unwrap();

        assert!(repo.is_local());
        assert!(!repo.is_git());
        assert!(repo.local_path().is_some());
    }

    #[test]
    fn detect_local_source_with_relative_path() {
        let system = RealSystem::new();
        // Create a temporary directory in current working directory
        let temp_dir = TempDir::new_in(".").unwrap();
        let dir_name = temp_dir.path().file_name().unwrap().to_str().unwrap();
        let relative_path = format!("file:./{dir_name}");

        let repo = Repository::new(&system, &relative_path).unwrap();

        assert!(repo.is_local());
        assert!(!repo.is_git());
        assert!(repo.local_path().is_some());
    }

    #[test]
    fn local_source_nonexistent_path() {
        let system = RealSystem::new();
        let result = Repository::new(&system, "file:///nonexistent/path/that/does/not/exist");
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.to_string().contains("does not exist"));
    }

    #[test]
    fn local_source_file_not_directory() {
        let system = RealSystem::new();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        system.write(&file_path, b"test").unwrap();

        let url = format!("file://{}", file_path.display());
        let result = Repository::new(&system, &url);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not a directory"));
    }

    #[test]
    fn repository_methods() {
        let system = RealSystem::new();

        // Test Git repository methods
        let repo = Repository::new(&system, "my_organization/repo").unwrap();
        assert_eq!(repo.original_url(), "my_organization/repo");
        assert_eq!(
            repo.git_url().unwrap(),
            "https://github.com/my_organization/repo.git"
        );
        assert_eq!(repo.local_path(), None);

        // Test local repository methods
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let url = format!("file://{}", path.display());
        let new_repo = Repository::new(&system, &url).unwrap();

        assert_eq!(new_repo.original_url(), &url);
        assert!(new_repo.local_path().is_some());
        assert_eq!(new_repo.local_path().unwrap(), path);
    }
}
