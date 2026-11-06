//! Checks on a loaded configuration.
//!
//! They cover repository formats, pull fields, path safety, replacements and child paths.

use pollster::FutureExt as _;
use std::path::{Path, PathBuf};

use crate::cli::{PullConfig, ReplacementConfig};
use crate::config::Config;
use crate::git::is_local_source;
use anyhow::{Result, anyhow};
use os_shim::System;
use regex::Regex;

/// Validates a configuration, resolving child paths against the current directory; one loaded from
/// another directory needs [`validate_config_with_base_dir`].
///
/// # Errors
///
/// Returns an error if there is no pull or child, or a repository, pull or child path is invalid.
#[inline]
pub fn validate_config(system: &dyn System, config: &Config) -> Result<()> {
    validate_config_with_base_dir(system, config, None)
}

/// Validates a configuration, resolving child paths against `base_dir`, or the current directory
/// when `None`.
///
/// # Errors
///
/// Returns an error if there is no pull or child, or a repository, pull or child path is invalid.
#[inline]
pub fn validate_config_with_base_dir(
    system: &dyn System,
    config: &Config,
    base_dir: Option<&Path>,
) -> Result<()> {
    if let Some(repo) = config.repository.as_ref() {
        validate_repository_url(repo)?;
    }

    if config.pulls.is_empty() && config.children.is_empty() {
        return Err(anyhow!(
            "Configuration must contain at least one pull operation or one child config"
        ));
    }

    for (index, pull) in config.pulls.iter().enumerate() {
        validate_pull_config(system, pull, index)?;
    }

    for (index, child_path) in config.children.iter().enumerate() {
        validate_child_path(system, child_path, index, base_dir)?;
    }

    Ok(())
}

/// Checks the fields of the pull at `index`, which labels its errors counting from one.
fn validate_pull_config(system: &dyn System, pull: &PullConfig, index: usize) -> Result<()> {
    let display_index = index.saturating_add(1);
    let context = format!("Pull operation #{display_index}");

    if let Some(repo) = pull.repository.as_ref() {
        validate_repository_url(repo).map_err(|err| anyhow!("{context}: {err}"))?;
    }

    if pull.source.trim().is_empty() {
        return Err(anyhow!("{context}: Source path cannot be empty"));
    }

    if pull.target.trim().is_empty() {
        return Err(anyhow!("{context}: Target path cannot be empty"));
    }

    if !matches!(pull.pull_type.as_str(), "file" | "directory") {
        return Err(anyhow!(
            "{}: Invalid pull type '{}'. Must be 'file' or 'directory'",
            context,
            pull.pull_type
        ));
    }

    validate_path_safety(&pull.target).map_err(|err| anyhow!("{context}: {err}"))?;

    for (cmd_index, command) in pull.commands.iter().enumerate() {
        if command.trim().is_empty() {
            let display_cmd_index = cmd_index.saturating_add(1);
            return Err(anyhow!(
                "{context}: Command #{display_cmd_index} cannot be empty"
            ));
        }
    }

    for (repl_index, replacement) in pull.replacements.iter().enumerate() {
        validate_replacement(system, replacement, index, repl_index)?;
    }

    Ok(())
}

/// Keeps a child inside the parent's tree: no `..`, not absolute, and the file must exist, resolved
/// against `base_dir` or the current directory.
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

    let resolved = base_dir.map_or_else(|| PathBuf::from(path), |dir| dir.join(path));
    if !system.exists(&resolved).block_on()? {
        return Err(anyhow!("{context}: Child config file not found: '{path}'"));
    }

    Ok(())
}

/// Accepts a `.git` HTTP or SSH URL, a short `account/repo`, or any local source, whose path
/// `Repository::new` checks later.
///
/// # Errors
///
/// Returns an error listing the supported formats if `url` matches none of them.
#[inline]
pub fn validate_repository_url(url: &str) -> Result<()> {
    if is_local_source(url) {
        return Ok(());
    }

    let patterns = [
        // Stricter than `normalize_repository_url`, which would complete a URL with no `.git`.
        r"^https?://.*\.git$",
        r"^git@.*\.git$",
        r"^[\w-]+/[\w-]+$",
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
        - Local: file:/path/to/repo, file:///path/to/repo, ~/path/to/repo, ./relative/path, or /absolute/path"
    ))
}

/// Rejects any path containing `..` and any absolute path.
///
/// # Errors
///
/// Returns an error if the path contains `..` or is absolute.
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

/// Requires a non-empty source and exactly one value source; a named environment variable must be
/// set at validation time.
fn validate_replacement(
    system: &dyn System,
    replacement: &ReplacementConfig,
    pull_index: usize,
    repl_index: usize,
) -> Result<()> {
    let display_pull = pull_index.saturating_add(1);
    let display_repl = repl_index.saturating_add(1);
    let context = format!("Pull #{display_pull}, Replacement #{display_repl}");

    if replacement.source.trim().is_empty() {
        return Err(anyhow!("{context}: Replacement source cannot be empty"));
    }

    match (
        replacement.target.as_ref(),
        replacement.value_from_env.as_ref(),
    ) {
        (Some(target), None) => {
            if target.trim().is_empty() {
                return Err(anyhow!("{context}: Replacement target cannot be empty"));
            }
        }
        (None, Some(env_var)) => {
            if env_var.trim().is_empty() {
                return Err(anyhow!(
                    "{context}: Environment variable name cannot be empty"
                ));
            }

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
