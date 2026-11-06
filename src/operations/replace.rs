//! Replaces text in the files of a pull's target, skipping binary files.
//!
//! A value is static or from the environment; `.graft.yaml` replacements may also use context.

use crate::cli::ReplacementConfig;
use crate::config::context::{ContextValues, value_to_string};
use crate::config::graft_yaml::GraftReplacement;
use crate::error::GraftError;
use crate::utils::fs::is_binary_file;
use anyhow::{Context as _, Result};
use os_shim::System;
use os_shim::bucket::Condition;
use pollster::FutureExt as _;
use regex::Regex;
use std::path::{Path, PathBuf};
use tracing::debug;

/// What one replacement would change, without changing it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ReplacementPreview {
    /// The text files that contain the pattern.
    pub affected_files: Vec<PathBuf>,
    /// The resolved value, after any environment lookup.
    pub replacement_value: String,
    /// The literal text searched for.
    pub search_pattern: String,
}

/// Applies each replacement under `target_dir` and returns the files changed, counted once per
/// replacement.
///
/// # Errors
///
/// Returns an error if the target is missing, a value is unresolved or a file cannot be updated.
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
    if !system.exists(target_path).block_on()? {
        return Err(GraftError::filesystem(format!(
            "Target directory does not exist: {target_dir}"
        ))
        .into());
    }

    let mut total_replacements: usize = 0;

    for replacement in replacements {
        let replacement_value = get_replacement_value(system, replacement)?;
        let files_processed =
            apply_single_replacement(system, target_path, &replacement.source, &replacement_value)?;
        total_replacements = total_replacements.saturating_add(files_processed);
    }

    Ok(total_replacements)
}

/// Applies `.graft.yaml` replacements, whose values may come from `context`, and returns the files
/// changed, counted once per replacement.
///
/// # Errors
///
/// Returns an error if the target is missing, a value is unresolved or a file cannot be updated.
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
    if !system.exists(target_path).block_on()? {
        return Err(GraftError::filesystem(format!(
            "Target directory does not exist: {target_dir}"
        ))
        .into());
    }

    let mut total_replacements: usize = 0;

    for replacement in replacements {
        let replacement_value = get_graft_replacement_value(system, replacement, context)?;
        let files_processed =
            apply_single_replacement(system, target_path, &replacement.source, &replacement_value)?;
        total_replacements = total_replacements.saturating_add(files_processed);
    }

    Ok(total_replacements)
}

/// Applies the regex to a file or to every text file under a directory and returns the files
/// changed.
///
/// # Errors
///
/// Returns an error if the regex is invalid or a file cannot be updated.
#[inline]
pub fn apply_regex_replacement(
    system: &dyn System,
    target_path: &Path,
    pattern: &str,
    replacement: &str,
) -> Result<usize> {
    let regex = Regex::new(pattern).with_context(|| format!("Invalid regex pattern: {pattern}"))?;

    let mut files_processed: usize = 0;

    if system.is_file(target_path).block_on()? {
        if apply_regex_to_file(system, target_path, &regex, replacement)? {
            files_processed = files_processed.saturating_add(1);
        }
    } else if system.is_dir(target_path).block_on()? {
        files_processed = files_processed.saturating_add(walk_and_apply_regex(
            system,
            target_path,
            &regex,
            replacement,
        )?);
    } else {
        debug!("Skipping file: {}", target_path.display());
    }

    Ok(files_processed)
}

/// Replaces literal text in a file or in every text file under a directory and returns the files
/// changed.
///
/// # Errors
///
/// Returns an error if a file cannot be updated.
#[inline]
pub fn apply_single_replacement(
    system: &dyn System,
    target_path: &Path,
    search_pattern: &str,
    replacement_value: &str,
) -> Result<usize> {
    let mut files_processed: usize = 0;

    if system.is_file(target_path).block_on()? {
        if apply_replacement_to_file(system, target_path, search_pattern, replacement_value)? {
            files_processed = files_processed.saturating_add(1);
        }
    } else if system.is_dir(target_path).block_on()? {
        files_processed = files_processed.saturating_add(walk_and_apply(
            system,
            target_path,
            search_pattern,
            replacement_value,
        )?);
    } else {
        debug!("Skipping file: {}", target_path.display());
    }
    Ok(files_processed)
}

/// Returns the static target or the value of the named environment variable.
///
/// # Errors
///
/// Returns an error if the variable is unset or the replacement does not set exactly one source.
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

/// Returns the static target, the named environment variable or the named context property.
///
/// # Errors
///
/// Returns an error unless exactly one source is set and it resolves to a string.
#[inline]
pub fn get_graft_replacement_value(
    system: &dyn System,
    replacement: &GraftReplacement,
    context: &ContextValues,
) -> Result<String> {
    let mut sources = 0_i32;
    if replacement.target.is_some() {
        sources = sources.saturating_add(1_i32);
    }
    if replacement.value_from_env.is_some() {
        sources = sources.saturating_add(1_i32);
    }
    if replacement.value_from_context.is_some() {
        sources = sources.saturating_add(1_i32);
    }

    if sources != 1_i32 {
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

        return value_to_string(value).map_err(|err| {
            GraftError::configuration(format!(
                "Failed to convert context property '{context_key}' to string: {err}"
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

/// Reports each replacement's value and the files holding its pattern, changing nothing.
///
/// # Errors
///
/// Returns an error if a value cannot be resolved or a file cannot be read.
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
            affected_files: files,
            replacement_value,
            search_pattern: replacement.source.clone(),
        });
    }

    Ok(previews)
}

/// Replaces literal text in one file and reports whether it changed; a binary file is skipped.
fn apply_replacement_to_file(
    system: &dyn System,
    file_path: &Path,
    search_pattern: &str,
    replacement_value: &str,
) -> Result<bool> {
    if is_binary_file(system, file_path)? {
        return Ok(false);
    }

    let content = system
        .read_to_string(file_path)
        .block_on()
        .with_context(|| {
            format!(
                "Failed to read file for text replacement: {}",
                file_path.display()
            )
        })?;

    if !content.contains(search_pattern) {
        return Ok(false);
    }

    let new_content = content.replace(search_pattern, replacement_value);

    if new_content != content {
        system
            .write(file_path, new_content.as_bytes(), Condition::None)
            .block_on()
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

/// Applies the regex to one file and reports whether it changed; a binary file is skipped.
fn apply_regex_to_file(
    system: &dyn System,
    file_path: &Path,
    regex: &Regex,
    replacement: &str,
) -> Result<bool> {
    if is_binary_file(system, file_path)? {
        return Ok(false);
    }

    let content = system
        .read_to_string(file_path)
        .block_on()
        .with_context(|| {
            format!(
                "Failed to read file for regex replacement: {}",
                file_path.display()
            )
        })?;

    let new_content = regex.replace_all(&content, replacement);

    if new_content != content {
        system
            .write(file_path, new_content.as_ref().as_bytes(), Condition::None)
            .block_on()
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

/// Looks for literal text in one file; a binary file never matches.
fn file_contains_pattern(system: &dyn System, file_path: &Path, pattern: &str) -> Result<bool> {
    if is_binary_file(system, file_path)? {
        return Ok(false);
    }

    let content = system
        .read_to_string(file_path)
        .block_on()
        .with_context(|| {
            format!(
                "Failed to read file for pattern check: {}",
                file_path.display()
            )
        })?;

    Ok(content.contains(pattern))
}

/// Lists the text files at or under `target_path` that contain the literal text.
fn find_files_with_pattern(
    system: &dyn System,
    target_path: &Path,
    pattern: &str,
) -> Result<Vec<PathBuf>> {
    let mut matching_files = Vec::new();

    if system.is_file(target_path).block_on()? {
        if file_contains_pattern(system, target_path, pattern)? {
            matching_files.push(target_path.to_path_buf());
        }
    } else if system.is_dir(target_path).block_on()? {
        find_files_recursive(system, target_path, pattern, &mut matching_files)?;
    } else {
        debug!("Skipping file: {}", target_path.display());
    }

    Ok(matching_files)
}

/// Appends to `matching_files` each text file under `dir_path` that contains the literal text.
fn find_files_recursive(
    system: &dyn System,
    dir_path: &Path,
    pattern: &str,
    matching_files: &mut Vec<PathBuf>,
) -> Result<()> {
    let entries = system
        .read_dir(dir_path)
        .block_on()
        .with_context(|| format!("Failed to read directory: {}", dir_path.display()))?;

    for entry_path in entries {
        if system.is_file(&entry_path).block_on()?
            && file_contains_pattern(system, &entry_path, pattern)?
        {
            matching_files.push(entry_path);
        } else if system.is_dir(&entry_path).block_on()? {
            find_files_recursive(system, &entry_path, pattern, matching_files)?;
        } else {
            debug!("Skipping directory: {}", entry_path.display());
        }
    }

    Ok(())
}

/// Replaces literal text in every text file under `dir_path` and returns how many changed.
fn walk_and_apply(
    system: &dyn System,
    dir_path: &Path,
    search_pattern: &str,
    replacement_value: &str,
) -> Result<usize> {
    let mut files_processed: usize = 0;

    let entries = system
        .read_dir(dir_path)
        .block_on()
        .with_context(|| format!("Failed to read directory: {}", dir_path.display()))?;

    for entry_path in entries {
        if system.is_file(&entry_path).block_on()? {
            if apply_replacement_to_file(system, &entry_path, search_pattern, replacement_value)? {
                files_processed = files_processed.saturating_add(1);
            }
        } else if system.is_dir(&entry_path).block_on()? {
            files_processed = files_processed.saturating_add(walk_and_apply(
                system,
                &entry_path,
                search_pattern,
                replacement_value,
            )?);
        } else {
            debug!("Skipping directory: {}", entry_path.display());
        }
    }

    Ok(files_processed)
}

/// Applies the regex to every text file under `dir_path` and returns how many changed.
fn walk_and_apply_regex(
    system: &dyn System,
    dir_path: &Path,
    regex: &Regex,
    replacement: &str,
) -> Result<usize> {
    let mut files_processed: usize = 0;

    let entries = system
        .read_dir(dir_path)
        .block_on()
        .with_context(|| format!("Failed to read directory: {}", dir_path.display()))?;

    for entry_path in entries {
        if system.is_file(&entry_path).block_on()? {
            if apply_regex_to_file(system, &entry_path, regex, replacement)? {
                files_processed = files_processed.saturating_add(1);
            }
        } else if system.is_dir(&entry_path).block_on()? {
            files_processed = files_processed.saturating_add(walk_and_apply_regex(
                system,
                &entry_path,
                regex,
                replacement,
            )?);
        } else {
            debug!("Skipping directory: {}", entry_path.display());
        }
    }

    Ok(files_processed)
}
