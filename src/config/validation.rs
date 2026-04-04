//! Configuration validation logic.

use std::path::{Path, PathBuf};

use crate::cli::{PullConfig, ReplacementConfig};
use crate::config::Config;
use anyhow::{Result, anyhow};
use os_shim::System;
use regex::Regex;

/// Validate a complete configuration.
///
/// Children paths are resolved relative to CWD. For configs loaded from
/// a child directory, use [`validate_config_with_base_dir`] instead.
///
/// # Errors
///
/// Returns an error if:
/// - The configuration does not contain at least one pull operation or child config
/// - The repository URL is invalid
/// - The pull configuration is invalid
/// - A child config path is invalid
#[inline]
pub fn validate_config(system: &dyn System, config: &Config) -> Result<()> {
    validate_config_with_base_dir(system, config, None)
}

/// Validate a complete configuration, resolving children paths relative to
/// `base_dir` (or CWD when `None`).
///
/// # Errors
///
/// Returns an error if:
/// - The configuration does not contain at least one pull operation or child config
/// - The repository URL is invalid
/// - The pull configuration is invalid
/// - A child config path is invalid
#[inline]
pub fn validate_config_with_base_dir(
    system: &dyn System,
    config: &Config,
    base_dir: Option<&Path>,
) -> Result<()> {
    // Validate global repository if present
    if let Some(repo) = config.repository.as_ref() {
        validate_repository_url(repo)?;
    }

    // Must have at least pulls or children
    if config.pulls.is_empty() && config.children.is_empty() {
        return Err(anyhow!(
            "Configuration must contain at least one pull operation or one child config"
        ));
    }

    // Validate each pull configuration
    for (index, pull) in config.pulls.iter().enumerate() {
        validate_pull_config(system, pull, index)?;
    }

    // Validate children paths
    for (index, child_path) in config.children.iter().enumerate() {
        validate_child_path(system, child_path, index, base_dir)?;
    }

    Ok(())
}

/// Validate a single pull configuration.
fn validate_pull_config(system: &dyn System, pull: &PullConfig, index: usize) -> Result<()> {
    let display_index = index.saturating_add(1);
    let context = format!("Pull operation #{display_index}");

    // Validate repository URL if present
    if let Some(repo) = pull.repository.as_ref() {
        validate_repository_url(repo).map_err(|err| anyhow!("{context}: {err}"))?;
    }

    // Validate source path
    if pull.source.trim().is_empty() {
        return Err(anyhow!("{context}: Source path cannot be empty"));
    }

    // Validate target path
    if pull.target.trim().is_empty() {
        return Err(anyhow!("{context}: Target path cannot be empty"));
    }

    // Validate pull type
    if !matches!(pull.pull_type.as_str(), "file" | "directory") {
        return Err(anyhow!(
            "{}: Invalid pull type '{}'. Must be 'file' or 'directory'",
            context,
            pull.pull_type
        ));
    }

    // Validate path safety (prevent path traversal)
    validate_path_safety(&pull.target).map_err(|err| anyhow!("{context}: {err}"))?;

    // Validate commands
    for (cmd_index, command) in pull.commands.iter().enumerate() {
        if command.trim().is_empty() {
            let display_cmd_index = cmd_index.saturating_add(1);
            return Err(anyhow!(
                "{context}: Command #{display_cmd_index} cannot be empty"
            ));
        }
    }

    // Validate replacements
    for (repl_index, replacement) in pull.replacements.iter().enumerate() {
        validate_replacement(system, replacement, index, repl_index)?;
    }

    Ok(())
}

/// Validate a child config path.
///
/// Children must be descendants of the parent config directory:
/// - No `..` in path (prevents escaping parent tree)
/// - No absolute paths (starting with `/`)
/// - File must exist
///
/// When `base_dir` is `Some`, the existence check resolves the path
/// relative to that directory instead of CWD.
fn validate_child_path(
    system: &dyn System,
    path: &str,
    index: usize,
    base_dir: Option<&Path>,
) -> Result<()> {
    let display_index = index.saturating_add(1);
    let context = format!("Child config #{display_index}");

    if path.trim().is_empty() {
        return Err(anyhow!("{context}: Child path cannot be empty"));
    }

    if path.contains("..") {
        return Err(anyhow!(
            "{context}: Child path '{path}' cannot contain '..' \u{2014} children must be in subdirectories"
        ));
    }

    if path.starts_with('/') {
        return Err(anyhow!(
            "{context}: Child path '{path}' cannot be absolute \u{2014} children must be in subdirectories"
        ));
    }

    // Check file exists, resolving relative to base_dir when provided
    let resolved = base_dir.map_or_else(|| PathBuf::from(path), |dir| dir.join(path));
    if !system.exists(&resolved)? {
        return Err(anyhow!("{context}: Child config file not found: '{path}'"));
    }

    Ok(())
}

/// Validate a repository URL format.
///
/// # Errors
///
/// Returns an error if:
/// - The repository URL is invalid
#[inline]
pub fn validate_repository_url(url: &str) -> Result<()> {
    // ONLY accept "file:" prefix for local filesystem paths
    if url.starts_with("file:") {
        // Local path - detailed validation will be done in Repository::new()
        return Ok(());
    }

    // Patterns for valid Git repository URLs
    let patterns = [
        r"^https?://.*\.git$", // HTTPS: https://github.com/user/repo.git
        r"^git@.*\.git$",      // SSH: git@github.com:user/repo.git
        r"^[\w-]+/[\w-]+$",    // Short: user/repo
    ];

    for pattern in &patterns {
        let regex = Regex::new(pattern)?;
        if regex.is_match(url) {
            return Ok(());
        }
    }

    Err(anyhow!(
        "Invalid repository URL format: '{url}'\n\
        Supported formats:\n\
        - Short format: my_organization/repo\n\
        - HTTPS: https://github.com/my_organization/repo.git\n\
        - SSH: git@github.com:my_organization/repo.git\n\
        - Local: file:/path/to/repo or file:///path/to/repo"
    ))
}

/// Validate path safety (prevent directory traversal).
///
/// # Errors
///
/// Returns an error if:
/// - The path contains unsafe directory traversal
/// - The path is an absolute path
#[inline]
pub fn validate_path_safety(path: &str) -> Result<()> {
    if path.contains("..") {
        return Err(anyhow!(
            "Path contains unsafe directory traversal: '{path}'"
        ));
    }

    if path.starts_with('/') && !path.starts_with("./") && !path.starts_with("../") {
        return Err(anyhow!(
            "Absolute paths are not allowed: '{path}'. Use relative paths instead."
        ));
    }

    Ok(())
}

/// Validate a text replacement configuration.
fn validate_replacement(
    system: &dyn System,
    replacement: &ReplacementConfig,
    pull_index: usize,
    repl_index: usize,
) -> Result<()> {
    let display_pull = pull_index.saturating_add(1);
    let display_repl = repl_index.saturating_add(1);
    let context = format!("Pull #{display_pull}, Replacement #{display_repl}");

    // Source must not be empty
    if replacement.source.trim().is_empty() {
        return Err(anyhow!("{context}: Replacement source cannot be empty"));
    }

    // Must have exactly one of target or value_from_env
    match (
        replacement.target.as_ref(),
        replacement.value_from_env.as_ref(),
    ) {
        (Some(target), None) => {
            // String literal replacement - target can be empty
            if target.trim().is_empty() {
                return Err(anyhow!("{context}: Replacement target cannot be empty"));
            }
        }
        (None, Some(env_var)) => {
            // Environment variable replacement
            if env_var.trim().is_empty() {
                return Err(anyhow!(
                    "{context}: Environment variable name cannot be empty"
                ));
            }

            // Check if environment variable exists
            if system.env_var(env_var).is_err() {
                return Err(anyhow!(
                    "{context}: Environment variable '{env_var}' is not set"
                ));
            }
        }
        (Some(_), Some(_)) => {
            return Err(anyhow!(
                "{context}: Cannot specify both 'target' and 'valueFromEnv'"
            ));
        }
        (None, None) => {
            return Err(anyhow!(
                "{context}: Must specify either 'target' or 'valueFromEnv'"
            ));
        }
    }

    Ok(())
}
