//! Configuration validation logic

use crate::cli::{PullConfig, ReplacementConfig};
use crate::config::Config;
use crate::system::System;
use anyhow::{Result, anyhow};
use regex::Regex;

/// Validate a complete configuration
pub fn validate_config(system: &dyn System, config: &Config) -> Result<()> {
    // Validate global repository if present
    if let Some(ref repo) = config.repository {
        validate_repository_url(repo)?;
    }

    // Validate that we have at least one pull
    if config.pulls.is_empty() {
        return Err(anyhow!(
            "Configuration must contain at least one pull operation"
        ));
    }

    // Validate each pull configuration
    for (index, pull) in config.pulls.iter().enumerate() {
        validate_pull_config(system, pull, index)?;
    }

    Ok(())
}

/// Validate a single pull configuration
fn validate_pull_config(system: &dyn System, pull: &PullConfig, index: usize) -> Result<()> {
    let context = format!("Pull operation #{}", index + 1);

    // Validate repository URL if present
    if let Some(repo) = &pull.repository {
        validate_repository_url(repo).map_err(|e| anyhow!("{context}: {e}"))?;
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
    validate_path_safety(&pull.target).map_err(|e| anyhow!("{context}: {e}"))?;

    // Validate commands
    for (cmd_index, command) in pull.commands.iter().enumerate() {
        if command.trim().is_empty() {
            return Err(anyhow!(
                "{}: Command #{} cannot be empty",
                context,
                cmd_index + 1
            ));
        }
    }

    // Validate replacements
    for (repl_index, replacement) in pull.replacements.iter().enumerate() {
        validate_replacement(system, replacement, index, repl_index)?;
    }

    Ok(())
}

/// Validate a repository URL format
fn validate_repository_url(url: &str) -> Result<()> {
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
        let regex = Regex::new(pattern).unwrap();
        if regex.is_match(url) {
            return Ok(());
        }
    }

    return Err(anyhow!(
        "Invalid repository URL format: '{url}'\n\
        Supported formats:\n\
        - Short format: myorg/repo\n\
        - HTTPS: https://github.com/myorg/repo.git\n\
        - SSH: git@github.com:myorg/repo.git\n\
        - Local: file:/path/to/repo or file:///path/to/repo"
    ));
}

/// Validate path safety (prevent directory traversal)
fn validate_path_safety(path: &str) -> Result<()> {
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

/// Validate a text replacement configuration
fn validate_replacement(
    system: &dyn System,
    replacement: &ReplacementConfig,
    pull_index: usize,
    repl_index: usize,
) -> Result<()> {
    let context = format!("Pull #{}, Replacement #{}", pull_index + 1, repl_index + 1);

    // Source must not be empty
    if replacement.source.trim().is_empty() {
        return Err(anyhow!("{context}: Replacement source cannot be empty"));
    }

    // Must have exactly one of target or value_from_env
    match (&replacement.target, &replacement.value_from_env) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_repository_url() {
        // Valid Git URLs
        assert!(validate_repository_url("myorg/repo").is_ok());
        assert!(validate_repository_url("https://github.com/myorg/repo.git").is_ok());
        assert!(validate_repository_url("git@github.com:myorg/repo.git").is_ok());

        // Valid local paths (ONLY file: prefix)
        assert!(validate_repository_url("file:///path/to/repo").is_ok());
        assert!(validate_repository_url("file:/path/to/repo").is_ok());
        assert!(validate_repository_url("file:~/src/repo").is_ok());

        // Invalid URLs
        assert!(validate_repository_url("invalid-url").is_err());
        assert!(validate_repository_url("").is_err());

        // Paths without file: prefix should now be rejected
        assert!(validate_repository_url("~/src/repo").is_err());
        assert!(validate_repository_url("./local/repo").is_err());
        assert!(validate_repository_url("../local/repo").is_err());
        assert!(validate_repository_url("/absolute/path/to/repo").is_err());
    }

    #[test]
    fn test_validate_path_safety() {
        // Safe paths
        assert!(validate_path_safety("./some/path").is_ok());
        assert!(validate_path_safety("some/path").is_ok());
        assert!(validate_path_safety("./relative/path").is_ok());

        // Unsafe paths
        assert!(validate_path_safety("../../unsafe").is_err());
        assert!(validate_path_safety("/absolute/path").is_err());
    }
}
