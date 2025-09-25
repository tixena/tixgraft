//! Git repository handling and URL parsing

use anyhow::Result;
use crate::error::GraftError;

/// Represents a Git repository with URL normalization
#[derive(Debug, Clone)]
pub struct Repository {
    pub url: String,
    pub normalized_url: String,
}

impl Repository {
    /// Create a new repository from a URL or short format
    pub fn new(url: &str) -> Result<Self> {
        let normalized_url = normalize_repository_url(url)?;
        
        Ok(Repository {
            url: url.to_string(),
            normalized_url,
        })
    }

    /// Get the normalized URL for Git operations
    pub fn git_url(&self) -> &str {
        &self.normalized_url
    }

    /// Get the original URL as provided
    pub fn original_url(&self) -> &str {
        &self.url
    }
}

/// Normalize a repository URL to a format suitable for Git operations
fn normalize_repository_url(url: &str) -> Result<String> {
    // Handle different URL formats
    if url.starts_with("https://") || url.starts_with("http://") {
        // Already a full HTTP/HTTPS URL
        if url.ends_with(".git") {
            Ok(url.to_string())
        } else {
            Ok(format!("{}.git", url))
        }
    } else if url.starts_with("git@") {
        // SSH URL - use as-is
        Ok(url.to_string())
    } else if url.contains('/') && !url.contains(':') {
        // Short format: myorg/repo -> https://github.com/myorg/repo.git
        if url.matches('/').count() == 1 {
            Ok(format!("https://github.com/{}.git", url))
        } else {
            Err(GraftError::configuration(format!(
                "Invalid repository format: '{}'. Expected format: 'org/repo'",
                url
            )).into())
        }
    } else {
        Err(GraftError::configuration(format!(
            "Unsupported repository URL format: '{}'\n\
            Supported formats:\n\
            - Short: myorg/repo\n\
            - HTTPS: https://github.com/myorg/repo.git\n\
            - SSH: git@github.com:myorg/repo.git",
            url
        )).into())
    }
}

/// Validate that a repository URL is accessible
pub fn validate_repository_access(repo: &Repository, tag: &str) -> Result<()> {
    // This is a placeholder for repository access validation
    // In a real implementation, this would attempt a shallow clone or ls-remote
    // to verify the repository exists and is accessible
    
    // For now, we'll just validate the URL format
    if repo.normalized_url.is_empty() {
        return Err(GraftError::git(
            "Repository URL cannot be empty".to_string()
        ).into());
    }

    if tag.is_empty() {
        return Err(GraftError::git(
            "Git reference (tag/branch) cannot be empty".to_string()
        ).into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
