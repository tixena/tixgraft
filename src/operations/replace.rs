//! Text replacement engine

use crate::cli::ReplacementConfig;
use crate::config::context::{ContextValues, value_to_string};
use crate::config::graft_yaml::GraftReplacement;
use crate::error::GraftError;
use crate::system::System;
use crate::utils::fs::is_binary_file;
use anyhow::{Context as _, Result};
use regex::Regex;
use tracing::debug;
use std::path::{Path, PathBuf};

/// Apply text replacements to files in the target directory
///
/// # Errors
///
/// Returns an error if:
/// - The target directory does not exist
/// - The replacements cannot be applied
#[inline]
pub fn apply_replacements(
    system: &dyn System,
    target_dir: &str,
    replacements: &[ReplacementConfig],
) -> Result<usize> {
    if replacements.is_empty() {
        return Ok(0);
    }

    let target_path = Path::new(target_dir);
    if !system.exists(target_path)? {
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
///
/// # Errors
///
/// Returns an error if:
/// - The replacement does not specify exactly one of target, valueFromEnv
#[inline]
pub fn get_replacement_value(
    system: &dyn System,
    replacement: &ReplacementConfig,
) -> Result<String> {
    match (
        replacement.target.as_ref(),
        replacement.value_from_env.as_ref(),
    ) {
        (Some(target), None) => Ok(target.clone()),
        (None, Some(env_var)) => system.env_var(env_var).map_err(|err| {
            GraftError::configuration(format!(
                "Environment variable '{env_var}' is not set. Error: {err}"
            ))
            .into()
        }),
        _ => Err(GraftError::configuration(
            "Replacement must specify exactly one of 'target' or 'valueFromEnv'".to_owned(),
        )
        .into()),
    }
}

/// Apply graft replacements (supports context) to files in the target directory
///
/// # Errors
///
/// Returns an error if:
/// - The target directory does not exist
/// - The replacements cannot be applied
#[inline]
pub fn apply_graft_replacements(
    system: &dyn System,
    target_dir: &str,
    replacements: &[GraftReplacement],
    context: &ContextValues,
) -> Result<usize> {
    if replacements.is_empty() {
        return Ok(0);
    }

    let target_path = Path::new(target_dir);
    if !system.exists(target_path)? {
        return Err(GraftError::filesystem(format!(
            "Target directory does not exist: {target_dir}"
        ))
        .into());
    }

    let mut total_replacements = 0;

    // Process each replacement
    for replacement in replacements {
        let replacement_value = get_graft_replacement_value(system, replacement, context)?;
        let files_processed =
            apply_single_replacement(system, target_path, &replacement.source, &replacement_value)?;
        total_replacements += files_processed;
    }

    Ok(total_replacements)
}

/// Get the replacement value from a `GraftReplacement` (supports context, env, or static)
///
/// # Errors
///
/// Returns an error if:
/// - The replacement does not specify exactly one of target, valueFromEnv, or valueFromContext
#[inline]
pub fn get_graft_replacement_value(
    system: &dyn System,
    replacement: &GraftReplacement,
    context: &ContextValues,
) -> Result<String> {
    let mut sources = 0;
    if replacement.target.is_some() {
        sources += 1;
    }
    if replacement.value_from_env.is_some() {
        sources += 1;
    }
    if replacement.value_from_context.is_some() {
        sources += 1;
    }

    if sources != 1 {
        return Err(GraftError::configuration(format!(
            "Replacement for '{}' must specify exactly one of: target, valueFromEnv, or valueFromContext",
            replacement.source
        ))
        .into());
    }

    if let Some(target) = replacement.target.as_ref() {
        return Ok(target.clone());
    }

    if let Some(env_var) = replacement.value_from_env.as_ref() {
        return system.env_var(env_var).map_err(|err| {
            GraftError::configuration(format!(
                "Environment variable '{env_var}' is not set. Error: {err}"
            ))
            .into()
        });
    }

    if let Some(context_key) = replacement.value_from_context.as_ref() {
        let value = context.get(context_key).ok_or_else(|| {
            GraftError::configuration(format!(
                "Context property '{}' not found for replacement of '{}'",
                context_key, replacement.source
            ))
        })?;

        return value_to_string(value).map_err(|e| {
            GraftError::configuration(format!(
                "Failed to convert context property '{context_key}' to string: {e}"
            ))
            .into()
        });
    }

    Err(GraftError::configuration(format!(
        "No replacement value specified for '{}'",
        replacement.source
    ))
    .into())
}

/// Apply a single replacement to all text files in the target directory
///
/// # Errors
///
/// Returns an error if:
/// - The replacements cannot be applied
#[inline]
pub fn apply_single_replacement(
    system: &dyn System,
    target_path: &Path,
    search_pattern: &str,
    replacement_value: &str,
) -> Result<usize> {
    let mut files_processed = 0;

    if system.is_file(target_path)? {
        // Single file case
        if apply_replacement_to_file(system, target_path, search_pattern, replacement_value)? {
            files_processed += 1;
        }
    } else if system.is_dir(target_path)? {
        // Directory case - recursively walk all files using System trait
        files_processed += walk_and_apply(system, target_path, search_pattern, replacement_value)?;
    }
    else {
        debug!("Skipping file: {}", target_path.display());
    }
    Ok(files_processed)
}

/// Recursively walk directory and apply replacements using System trait
fn walk_and_apply(
    system: &dyn System,
    dir_path: &Path,
    search_pattern: &str,
    replacement_value: &str,
) -> Result<usize> {
    let mut files_processed = 0;

    let entries = system
        .read_dir(dir_path)
        .with_context(|| format!("Failed to read directory: {}", dir_path.display()))?;

    for entry_path in entries {
        if system.is_file(&entry_path)? {
            if apply_replacement_to_file(system, &entry_path, search_pattern, replacement_value)? {
                files_processed += 1;
            }
        } else if system.is_dir(&entry_path)? {
            // Recursively process subdirectories
            files_processed +=
                walk_and_apply(system, &entry_path, search_pattern, replacement_value)?;
        }
        else {
            debug!("Skipping directory: {}", entry_path.display());
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
///
/// # Errors
///
/// Returns an error if:
/// - The regex pattern is invalid
/// - The replacements cannot be applied
#[inline]
pub fn apply_regex_replacement(
    system: &dyn System,
    target_path: &Path,
    pattern: &str,
    replacement: &str,
) -> Result<usize> {
    let regex = Regex::new(pattern).with_context(|| format!("Invalid regex pattern: {pattern}"))?;

    let mut files_processed = 0;

    if system.is_file(target_path)? {
        if apply_regex_to_file(system, target_path, &regex, replacement)? {
            files_processed += 1;
        }
    } else if system.is_dir(target_path)? {
        files_processed += walk_and_apply_regex(system, target_path, &regex, replacement)?;
    }
    else {
        debug!("Skipping file: {}", target_path.display());
    }

    Ok(files_processed)
}

/// Recursively walk directory and apply regex replacements using System trait
fn walk_and_apply_regex(
    system: &dyn System,
    dir_path: &Path,
    regex: &Regex,
    replacement: &str,
) -> Result<usize> {
    let mut files_processed = 0;

    let entries = system
        .read_dir(dir_path)
        .with_context(|| format!("Failed to read directory: {}", dir_path.display()))?;

    for entry_path in entries {
        if system.is_file(&entry_path)? {
            if apply_regex_to_file(system, &entry_path, regex, replacement)? {
                files_processed += 1;
            }
        } else if system.is_dir(&entry_path)? {
            files_processed += walk_and_apply_regex(system, &entry_path, regex, replacement)?;
        }
        else {
            debug!("Skipping directory: {}", entry_path.display());
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
///
/// # Errors
///
/// Returns an error if:
/// - The replacements cannot be previewed
#[inline]
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

    if system.is_file(target_path)? {
        if file_contains_pattern(system, target_path, pattern)? {
            matching_files.push(target_path.to_path_buf());
        }
    } else if system.is_dir(target_path)? {
        find_files_recursive(system, target_path, pattern, &mut matching_files)?;
    }
    else {
        debug!("Skipping file: {}", target_path.display());
    }

    Ok(matching_files)
}

/// Recursively find files containing pattern using System trait
fn find_files_recursive(
    system: &dyn System,
    dir_path: &Path,
    pattern: &str,
    matching_files: &mut Vec<PathBuf>,
) -> Result<()> {
    let entries = system
        .read_dir(dir_path)
        .with_context(|| format!("Failed to read directory: {}", dir_path.display()))?;

    for entry_path in entries {
        if system.is_file(&entry_path)? && file_contains_pattern(system, &entry_path, pattern)? {
            matching_files.push(entry_path);
        } else if system.is_dir(&entry_path)? {
            find_files_recursive(system, &entry_path, pattern, matching_files)?;
        }
        else {
            debug!("Skipping directory: {}", entry_path.display());
        }
    }

    Ok(())
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
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ReplacementPreview {
    pub search_pattern: String,
    pub replacement_value: String,
    pub affected_files: Vec<PathBuf>,
}
