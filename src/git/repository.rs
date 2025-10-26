//! Git repository handling and URL parsing

use crate::error::GraftError;
use crate::system::System;
use anyhow::Result;
use std::path::PathBuf;

/// Represents a repository source - either Git or local filesystem
#[derive(Debug, Clone)]
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
pub struct Repository {
    pub url: String,
    pub source: RepositorySource,
}

impl Repository {
    /// Create a new repository from a URL or local path
    pub fn new(system: &dyn System, url: &str) -> Result<Self> {
        let source = detect_source_type(system, url)?;

        return Ok(Self {
            url: url.to_owned(),
            source,
        });
    }

    /// Get the normalized URL for Git operations (panics if called on Local source)
    #[must_use]
    pub fn git_url(&self) -> &str {
        match &self.source {
            RepositorySource::Git { normalized_url, .. } => normalized_url,
            RepositorySource::Local { .. } => {
                panic!("git_url() called on Local repository source")
            }
        }
    }

    /// Get the original URL as provided
    #[must_use]
    pub fn original_url(&self) -> &str {
        &self.url
    }

    /// Check if this is a Git repository
    #[must_use]
    pub const fn is_git(&self) -> bool {
        matches!(self.source, RepositorySource::Git { .. })
    }

    /// Check if this is a local filesystem source
    #[must_use]
    pub const fn is_local(&self) -> bool {
        matches!(self.source, RepositorySource::Local { .. })
    }

    /// Get the local path (returns None if this is a Git source)
    #[must_use]
    pub const fn local_path(&self) -> Option<&PathBuf> {
        match &self.source {
            RepositorySource::Local { resolved_path, .. } => Some(resolved_path),
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
            url.strip_prefix("file://").unwrap()
        } else {
            url.strip_prefix("file:").unwrap()
        };
        return create_local_source(system, url, path_str);
    }

    // Everything else is treated as a Git repository
    let normalized_url = normalize_repository_url(url)?;
    return Ok(RepositorySource::Git {
        original_url: url.to_owned(),
        normalized_url,
    });
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
            .map_err(|_| {
                return GraftError::configuration(
                    "Cannot determine home directory for ~ expansion".to_owned(),
                );
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
            .map_err(|e| {
                return GraftError::filesystem(format!("Cannot get current directory: {e}"));
            })?
            .join(&path)
    };

    // Verify the path exists
    if !system.exists(&resolved_path) {
        return Err(GraftError::source(format!(
            "Local repository path does not exist: '{}'",
            resolved_path.display()
        ))
        .into());
    }

    // Verify it's a directory
    if !system.is_dir(&resolved_path) {
        return Err(GraftError::source(format!(
            "Local repository path is not a directory: '{}'",
            resolved_path.display()
        ))
        .into());
    }

    return Ok(RepositorySource::Local {
        original_path: original.to_owned(),
        resolved_path,
    });
}

/// Normalize a repository URL to a format suitable for Git operations
fn normalize_repository_url(url: &str) -> Result<String> {
    // Handle different URL formats
    if url.starts_with("https://") || url.starts_with("http://") {
        // Already a full HTTP/HTTPS URL
        if url.ends_with(".git") {
            return Ok(url.to_owned());
        } else {
            return Ok(format!("{url}.git"));
        }
    } else if url.starts_with("git@") {
        // SSH URL - use as-is
        return Ok(url.to_owned());
    } else if url.contains('/') && !url.contains(':') {
        // Short format: myorg/repo -> https://github.com/myorg/repo.git
        if url.matches('/').count() == 1 {
            return Ok(format!("https://github.com/{url}.git"));
        } else {
            return Err(GraftError::configuration(format!(
                "Invalid repository format: '{url}'. Expected format: 'org/repo'"
            ))
            .into());
        }
    } else {
        return Err(GraftError::configuration(format!(
            "Unsupported repository URL format: '{url}'\n\
            Supported formats:\n\
            - Short: myorg/repo\n\
            - HTTPS: https://github.com/myorg/repo.git\n\
            - SSH: git@github.com:myorg/repo.git\n\
            - Local: file:///path/to/repo or ~/path/to/repo"
        ))
        .into());
    }
}

/// Validate that a repository URL is accessible
pub fn validate_repository_access(repo: &Repository, tag: &str) -> Result<()> {
    // For local repositories, we've already validated the path exists in detect_source_type
    if repo.is_local() {
        // Local sources don't need tag validation (ignored)
        return Ok(());
    }

    // For Git repositories, validate URL and tag
    match &repo.source {
        RepositorySource::Git { normalized_url, .. } => {
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
mod tests {
    use super::*;
    use crate::system::RealSystem;
    use tempfile::TempDir;

    #[test]
    fn test_normalize_repository_url() {
        // Short format
        assert_eq!(
            normalize_repository_url("myorg/repo").unwrap(),
            "https://github.com/myorg/repo.git"
        );

        // HTTPS without .git
        assert_eq!(
            normalize_repository_url("https://github.com/myorg/repo").unwrap(),
            "https://github.com/myorg/repo.git"
        );

        // HTTPS with .git
        assert_eq!(
            normalize_repository_url("https://github.com/myorg/repo.git").unwrap(),
            "https://github.com/myorg/repo.git"
        );

        // SSH
        assert_eq!(
            normalize_repository_url("git@github.com:myorg/repo.git").unwrap(),
            "git@github.com:myorg/repo.git"
        );
    }

    #[test]
    fn test_invalid_repository_urls() {
        assert!(normalize_repository_url("invalid").is_err());
        assert!(normalize_repository_url("").is_err());
        assert!(normalize_repository_url("too/many/slashes").is_err());
    }

    #[test]
    fn test_detect_git_source() {
        let system = RealSystem::new();

        // Short format
        let repo = Repository::new(&system, "myorg/repo").unwrap();
        assert!(repo.is_git());
        assert!(!repo.is_local());
        assert_eq!(repo.git_url(), "https://github.com/myorg/repo.git");

        // HTTPS
        let repo = Repository::new(&system, "https://github.com/myorg/repo.git").unwrap();
        assert!(repo.is_git());
        assert!(!repo.is_local());

        // SSH
        let repo = Repository::new(&system, "git@github.com:myorg/repo.git").unwrap();
        assert!(repo.is_git());
        assert!(!repo.is_local());
    }

    #[test]
    fn test_detect_local_source_with_file_prefix() {
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
    fn test_detect_local_source_with_absolute_path() {
        let system = RealSystem::new();
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();
        let url = format!("file:{}", path);

        let repo = Repository::new(&system, &url).unwrap();

        assert!(repo.is_local());
        assert!(!repo.is_git());
        assert!(repo.local_path().is_some());
    }

    #[test]
    fn test_detect_local_source_with_relative_path() {
        let system = RealSystem::new();
        // Create a temporary directory in current working directory
        let temp_dir = TempDir::new_in(".").unwrap();
        let dir_name = temp_dir.path().file_name().unwrap().to_str().unwrap();
        let relative_path = format!("file:./{}", dir_name);

        let repo = Repository::new(&system, &relative_path).unwrap();

        assert!(repo.is_local());
        assert!(!repo.is_git());
        assert!(repo.local_path().is_some());
    }

    #[test]
    fn test_local_source_nonexistent_path() {
        let system = RealSystem::new();
        let result = Repository::new(&system, "file:///nonexistent/path/that/does/not/exist");
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.to_string().contains("does not exist"));
    }

    #[test]
    fn test_local_source_file_not_directory() {
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
    fn test_repository_methods() {
        let system = RealSystem::new();

        // Test Git repository methods
        let repo = Repository::new(&system, "myorg/repo").unwrap();
        assert_eq!(repo.original_url(), "myorg/repo");
        assert_eq!(repo.git_url(), "https://github.com/myorg/repo.git");
        assert_eq!(repo.local_path(), None);

        // Test local repository methods
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let url = format!("file://{}", path.display());
        let repo = Repository::new(&system, &url).unwrap();

        assert_eq!(repo.original_url(), &url);
        assert!(repo.local_path().is_some());
        assert_eq!(repo.local_path().unwrap(), path);
    }
}
