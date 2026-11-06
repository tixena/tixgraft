//! Classifies a source string as Git or local.
//!
//! A Git source gets a full URL to fetch from, and a local source a checked absolute path.

use crate::error::GraftError;
use anyhow::Result;
use os_shim::System;
use pollster::FutureExt as _;
use std::path::{Path, PathBuf};

/// Where a pull reads from: a remote Git repository or a local directory.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum RepositorySource {
    /// A repository fetched over Git.
    Git {
        /// Full URL to fetch from, with a short `org/repo` expanded to GitHub.
        normalized_url: String,
        /// URL exactly as the user wrote it.
        original_url: String,
    },
    /// A directory on the local filesystem, checked to exist when built.
    Local {
        /// Source exactly as the user wrote it, including any `file:` prefix.
        original_path: String,
        /// Absolute path with `file:` and `~` resolved.
        resolved_path: PathBuf,
    },
}

/// A classified source together with the text it came from.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Repository {
    /// Classification of `url`.
    pub source: RepositorySource,
    /// Source exactly as the user wrote it.
    pub url: String,
}

impl Repository {
    /// Returns the normalized URL to fetch from.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository is a local source.
    #[inline]
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "matching on &RepositorySource; dereferencing would require ref bindings which conflict with clippy::ref_patterns"
    )]
    pub fn git_url(&self) -> Result<&str> {
        match &self.source {
            RepositorySource::Git {
                normalized_url,
                original_url: _,
            } => Ok(normalized_url),
            RepositorySource::Local {
                original_path: _,
                resolved_path: _,
            } => Err(GraftError::git("git_url() called on Local repository source").into()),
        }
    }

    /// True for a source fetched over Git, including a short `org/repo`.
    #[must_use]
    #[inline]
    pub const fn is_git(&self) -> bool {
        matches!(
            self.source,
            RepositorySource::Git {
                normalized_url: _,
                original_url: _
            }
        )
    }

    /// True for a source read directly from disk, with no fetch or checkout.
    #[must_use]
    #[inline]
    pub const fn is_local(&self) -> bool {
        matches!(
            self.source,
            RepositorySource::Local {
                original_path: _,
                resolved_path: _
            }
        )
    }

    /// Returns the resolved absolute path, or `None` for a Git source.
    #[must_use]
    #[inline]
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "matching on &RepositorySource; dereferencing would require ref bindings which conflict with clippy::ref_patterns"
    )]
    pub const fn local_path(&self) -> Option<&PathBuf> {
        match &self.source {
            RepositorySource::Local {
                original_path: _,
                resolved_path,
            } => Some(resolved_path),
            RepositorySource::Git {
                normalized_url: _,
                original_url: _,
            } => None,
        }
    }

    /// Classifies `url` as Git or local; a local path must already exist as a directory.
    ///
    /// # Errors
    ///
    /// Returns an error if a Git form is malformed, or a local path is missing or not a directory.
    #[inline]
    pub fn new(system: &dyn System, url: &str) -> Result<Self> {
        let source = detect_source_type(system, url)?;

        Ok(Self {
            url: url.to_owned(),
            source,
        })
    }

    /// Returns the source as the user wrote it, which error messages quote.
    #[must_use]
    #[inline]
    pub fn original_url(&self) -> &str {
        &self.url
    }
}

/// A source is local when it starts with `file:`, `~`, `/`, `./` or `../`; everything else is Git.
pub(crate) fn is_local_source(url: &str) -> bool {
    url.starts_with("file:")
        || url.starts_with('~')
        || url.starts_with('/')
        || url.starts_with("./")
        || url.starts_with("../")
}

/// Builds the variant `url` selects, stripping any `file:` or `file://` scheme from a local source.
fn detect_source_type(system: &dyn System, url: &str) -> Result<RepositorySource> {
    if is_local_source(url) {
        let path_str = url
            .strip_prefix("file://")
            .or_else(|| url.strip_prefix("file:"))
            .unwrap_or(url);
        return create_local_source(system, url, path_str);
    }

    let normalized_url = normalize_repository_url(url)?;
    Ok(RepositorySource::Git {
        original_url: url.to_owned(),
        normalized_url,
    })
}

/// Expands `~`, makes the path absolute and requires an existing directory.
fn create_local_source(
    system: &dyn System,
    original: &str,
    path_str: &str,
) -> Result<RepositorySource> {
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

    let resolved_path = if path.is_absolute() {
        path
    } else {
        system
            .current_dir()
            .map_err(|err| GraftError::filesystem(format!("Cannot get current directory: {err}")))?
            .join(&path)
    };

    if !system.exists(&resolved_path).block_on()? {
        return Err(GraftError::from_source(format!(
            "Local repository path does not exist: '{}'",
            resolved_path.display()
        ))
        .into());
    }

    if !system.is_dir(&resolved_path).block_on()? {
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

/// Appends `.git` to an HTTP URL, keeps an SSH URL and expands a short `org/repo` to GitHub.
fn normalize_repository_url(url: &str) -> Result<String> {
    if url.starts_with("https://") || url.starts_with("http://") {
        if Path::new(url)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("git"))
        {
            Ok(url.to_owned())
        } else {
            Ok(format!("{url}.git"))
        }
    } else if url.starts_with("git@") {
        Ok(url.to_owned())
    } else if url.contains('/') && !url.contains(':') {
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
            - Local: file:/path/to/repo, file:///path/to/repo, ~/path/to/repo, ./relative/path, or /absolute/path"
        ))
        .into())
    }
}

/// Requires a non-empty URL and reference on a Git source, with no network call; a local source
/// passes, since `Repository::new` already checked its path.
///
/// # Errors
///
/// Returns an error if a Git source has an empty URL or an empty reference.
#[inline]
#[expect(
    clippy::pattern_type_mismatch,
    reason = "matching on &RepositorySource; dereferencing would require ref bindings which conflict with clippy::ref_patterns"
)]
pub fn validate_repository_access(repo: &Repository, tag: &str) -> Result<()> {
    if repo.is_local() {
        return Ok(());
    }

    match &repo.source {
        RepositorySource::Git {
            normalized_url,
            original_url: _,
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
        RepositorySource::Local {
            original_path: _,
            resolved_path: _,
        } => {}
    }

    Ok(())
}

#[cfg(test)]
mod tests;
