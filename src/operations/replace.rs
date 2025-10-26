//! Text replacement engine

use crate::cli::ReplacementConfig;
use crate::error::GraftError;
use crate::system::System;
use crate::utils::fs::is_binary_file;
use anyhow::{Context as _, Result};
use regex::Regex;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Apply text replacements to files in the target directory
pub fn apply_replacements(
    system: &dyn System,
    target_dir: &str,
    replacements: &[ReplacementConfig],
) -> Result<usize> {
    if replacements.is_empty() {
        return Ok(0);
    }

    let target_path = Path::new(target_dir);
    if !system.exists(target_path) {
        return Err(GraftError::filesystem(format!(
            "Target directory does not exist: {target_dir}"
        ))
        .into());
    }

    let mut total_replacements = 0;

    // Process each replacement
    for replacement in replacements {
        let replacement_value = get_replacement_value(system, replacement)?;
        let files_processed =
            apply_single_replacement(system, target_path, &replacement.source, &replacement_value)?;
        total_replacements += files_processed;
    }

    Ok(total_replacements)
}

/// Get the replacement value from either target or environment variable
fn get_replacement_value(system: &dyn System, replacement: &ReplacementConfig) -> Result<String> {
    match (&replacement.target, &replacement.value_from_env) {
        (Some(target), None) => Ok(target.clone()),
        (None, Some(env_var)) => {
            return system
                .env_var(env_var)
                .map_err(|_| anyhow::anyhow!("Environment variable '{env_var}' is not set"));
        }
        _ => {
            return Err(GraftError::configuration(
                "Replacement must specify exactly one of 'target' or 'valueFromEnv'".to_owned(),
            )
            .into());
        }
    }
}

/// Apply a single replacement to all text files in the target directory
fn apply_single_replacement(
    system: &dyn System,
    target_path: &Path,
    search_pattern: &str,
    replacement_value: &str,
) -> Result<usize> {
    let mut files_processed = 0;

    if system.is_file(target_path) {
        // Single file case
        if apply_replacement_to_file(system, target_path, search_pattern, replacement_value)? {
            files_processed += 1;
        }
    } else if system.is_dir(target_path) {
        // Directory case - walk all files
        for entry in WalkDir::new(target_path) {
            let entry = entry.context("Failed to read directory entry during replacement")?;

            if entry.file_type().is_file()
                && apply_replacement_to_file(
                    system,
                    entry.path(),
                    search_pattern,
                    replacement_value,
                )?
            {
                files_processed += 1;
            }
        }
    }

    Ok(files_processed)
}

/// Apply replacement to a single file
fn apply_replacement_to_file(
    system: &dyn System,
    file_path: &Path,
    search_pattern: &str,
    replacement_value: &str,
) -> Result<bool> {
    // Skip binary files
    if is_binary_file(system, file_path)? {
        return Ok(false);
    }

    // Read file content
    let content = system.read_to_string(file_path).with_context(|| {
        format!(
            "Failed to read file for text replacement: {}",
            file_path.display()
        )
    })?;

    // Check if the search pattern exists
    if !content.contains(search_pattern) {
        return Ok(false);
    }

    // Apply replacement
    let new_content = content.replace(search_pattern, replacement_value);

    // Only write if content actually changed
    if new_content != content {
        system
            .write(file_path, new_content.as_bytes())
            .with_context(|| {
                format!(
                    "Failed to write file after text replacement: {}",
                    file_path.display()
                )
            })?;

        return Ok(true);
    }

    Ok(false)
}

/// Apply regex-based replacements (advanced feature)
pub fn apply_regex_replacement(
    system: &dyn System,
    target_path: &Path,
    pattern: &str,
    replacement: &str,
) -> Result<usize> {
    let regex = Regex::new(pattern).with_context(|| format!("Invalid regex pattern: {pattern}"))?;

    let mut files_processed = 0;

    if system.is_file(target_path) {
        if apply_regex_to_file(system, target_path, &regex, replacement)? {
            files_processed += 1;
        }
    } else if system.is_dir(target_path) {
        for entry in WalkDir::new(target_path) {
            let entry = entry?;

            if entry.file_type().is_file()
                && apply_regex_to_file(system, entry.path(), &regex, replacement)?
            {
                files_processed += 1;
            }
        }
    }

    Ok(files_processed)
}

/// Apply regex replacement to a single file
fn apply_regex_to_file(
    system: &dyn System,
    file_path: &Path,
    regex: &Regex,
    replacement: &str,
) -> Result<bool> {
    // Skip binary files
    if is_binary_file(system, file_path)? {
        return Ok(false);
    }

    // Read file content
    let content = system.read_to_string(file_path).with_context(|| {
        format!(
            "Failed to read file for regex replacement: {}",
            file_path.display()
        )
    })?;

    // Apply regex replacement
    let new_content = regex.replace_all(&content, replacement);

    // Only write if content actually changed
    if new_content != content {
        system
            .write(file_path, new_content.as_ref().as_bytes())
            .with_context(|| {
                format!(
                    "Failed to write file after regex replacement: {}",
                    file_path.display()
                )
            })?;

        return Ok(true);
    }

    Ok(false)
}

/// Preview what replacements would be applied (for dry run)
pub fn preview_replacements(
    system: &dyn System,
    target_dir: &str,
    replacements: &[ReplacementConfig],
) -> Result<Vec<ReplacementPreview>> {
    let mut previews = Vec::new();
    let target_path = Path::new(target_dir);

    for replacement in replacements {
        let replacement_value = get_replacement_value(system, replacement)?;
        let files = find_files_with_pattern(system, target_path, &replacement.source)?;

        previews.push(ReplacementPreview {
            search_pattern: replacement.source.clone(),
            replacement_value,
            affected_files: files,
        });
    }

    Ok(previews)
}

/// Find all files that contain a specific pattern
fn find_files_with_pattern(
    system: &dyn System,
    target_path: &Path,
    pattern: &str,
) -> Result<Vec<PathBuf>> {
    let mut matching_files = Vec::new();

    if system.is_file(target_path) {
        if file_contains_pattern(system, target_path, pattern)? {
            matching_files.push(target_path.to_path_buf());
        }
    } else if system.is_dir(target_path) {
        for entry in WalkDir::new(target_path) {
            let entry = entry?;

            if entry.file_type().is_file() && file_contains_pattern(system, entry.path(), pattern)?
            {
                matching_files.push(entry.path().to_path_buf());
            }
        }
    }

    Ok(matching_files)
}

/// Check if a file contains a specific pattern
fn file_contains_pattern(system: &dyn System, file_path: &Path, pattern: &str) -> Result<bool> {
    // Skip binary files
    if is_binary_file(system, file_path)? {
        return Ok(false);
    }

    let content = system.read_to_string(file_path).with_context(|| {
        format!(
            "Failed to read file for pattern check: {}",
            file_path.display()
        )
    })?;

    Ok(content.contains(pattern))
}

/// Preview information for a replacement
#[derive(Debug)]
pub struct ReplacementPreview {
    pub search_pattern: String,
    pub replacement_value: String,
    pub affected_files: Vec<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::MockSystem;

    #[test]
    fn test_apply_simple_replacement() {
        let system =
            MockSystem::new().with_file("/test.txt", b"Hello {{NAME}}, welcome to {{PLACE}}!\n");

        // Create replacement config
        let replacement = ReplacementConfig {
            source: "{{NAME}}".to_string(),
            target: Some("Alice".to_string()),
            value_from_env: None,
        };

        // Apply replacement
        let result = apply_single_replacement(
            &system,
            std::path::Path::new("/test.txt"),
            &replacement.source,
            &replacement.target.as_ref().unwrap(),
        );

        assert!(result.is_ok());

        // Verify replacement was applied
        let content = system
            .read_to_string(std::path::Path::new("/test.txt"))
            .unwrap();
        assert!(content.contains("Hello Alice"));
        assert!(content.contains("{{PLACE}}"));
    }

    #[test]
    fn test_replacement_with_env_var() {
        let system = MockSystem::new().with_env("TEST_ENV", "TestValue");

        let replacement = ReplacementConfig {
            source: "{{TEST}}".to_string(),
            target: None,
            value_from_env: Some("TEST_ENV".to_string()),
        };

        let value = get_replacement_value(&system, &replacement);
        assert!(value.is_ok());
        assert_eq!(value.unwrap(), "TestValue");
    }
}
