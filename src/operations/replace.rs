//! Text replacement engine

use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Result, Context};
use walkdir::WalkDir;
use regex::Regex;
use crate::cli::ReplacementConfig;
use crate::error::GraftError;
use crate::utils::fs::is_binary_file;

/// Apply text replacements to files in the target directory
pub fn apply_replacements(target_dir: &str, replacements: &[ReplacementConfig]) -> Result<usize> {
    if replacements.is_empty() {
        return Ok(0);
    }

    let target_path = Path::new(target_dir);
    if !target_path.exists() {
        return Err(GraftError::filesystem(format!(
            "Target directory does not exist: {}",
            target_dir
        )).into());
    }

    let mut total_replacements = 0;

    // Process each replacement
    for replacement in replacements {
        let replacement_value = get_replacement_value(replacement)?;
        let files_processed = apply_single_replacement(target_path, &replacement.source, &replacement_value)?;
        total_replacements += files_processed;
    }

    Ok(total_replacements)
}

/// Get the replacement value from either target or environment variable
fn get_replacement_value(replacement: &ReplacementConfig) -> Result<String> {
    match (&replacement.target, &replacement.value_from_env) {
        (Some(target), None) => Ok(target.clone()),
        (None, Some(env_var)) => {
            std::env::var(env_var)
                .with_context(|| format!(
                    "Environment variable '{}' is not set",
                    env_var
                ))
        }
        _ => Err(GraftError::configuration(
            "Replacement must specify exactly one of 'target' or 'valueFromEnv'".to_string()
        ).into()),
    }
}

/// Apply a single replacement to all text files in the target directory
fn apply_single_replacement(target_path: &Path, search_pattern: &str, replacement_value: &str) -> Result<usize> {
    let mut files_processed = 0;

    if target_path.is_file() {
        // Single file case
        if apply_replacement_to_file(target_path, search_pattern, replacement_value)? {
            files_processed += 1;
        }
    } else if target_path.is_dir() {
        // Directory case - walk all files
        for entry in WalkDir::new(target_path) {
            let entry = entry.context("Failed to read directory entry during replacement")?;
            
            if entry.file_type().is_file() {
                if apply_replacement_to_file(entry.path(), search_pattern, replacement_value)? {
                    files_processed += 1;
                }
            }
        }
    }

    Ok(files_processed)
}

/// Apply replacement to a single file
fn apply_replacement_to_file(file_path: &Path, search_pattern: &str, replacement_value: &str) -> Result<bool> {
    // Skip binary files
    if is_binary_file(file_path)? {
        return Ok(false);
    }

    // Read file content
    let content = fs::read_to_string(file_path)
        .with_context(|| format!(
            "Failed to read file for text replacement: {}",
            file_path.display()
        ))?;

    // Check if the search pattern exists
    if !content.contains(search_pattern) {
        return Ok(false);
    }

    // Apply replacement
    let new_content = content.replace(search_pattern, replacement_value);

    // Only write if content actually changed
    if new_content != content {
        fs::write(file_path, new_content)
            .with_context(|| format!(
                "Failed to write file after text replacement: {}",
                file_path.display()
            ))?;
        
        return Ok(true);
    }

    Ok(false)
}

/// Apply regex-based replacements (advanced feature)
pub fn apply_regex_replacement(target_path: &Path, pattern: &str, replacement: &str) -> Result<usize> {
    let regex = Regex::new(pattern)
        .with_context(|| format!("Invalid regex pattern: {}", pattern))?;

    let mut files_processed = 0;

    if target_path.is_file() {
        if apply_regex_to_file(target_path, &regex, replacement)? {
            files_processed += 1;
        }
    } else if target_path.is_dir() {
        for entry in WalkDir::new(target_path) {
            let entry = entry?;
            
            if entry.file_type().is_file() {
                if apply_regex_to_file(entry.path(), &regex, replacement)? {
                    files_processed += 1;
                }
            }
        }
    }

    Ok(files_processed)
}

/// Apply regex replacement to a single file
fn apply_regex_to_file(file_path: &Path, regex: &Regex, replacement: &str) -> Result<bool> {
    // Skip binary files
    if is_binary_file(file_path)? {
        return Ok(false);
    }

    // Read file content
    let content = fs::read_to_string(file_path)
        .with_context(|| format!(
            "Failed to read file for regex replacement: {}",
            file_path.display()
        ))?;

    // Apply regex replacement
    let new_content = regex.replace_all(&content, replacement);

    // Only write if content actually changed
    if new_content != content {
        fs::write(file_path, new_content.as_ref())
            .with_context(|| format!(
                "Failed to write file after regex replacement: {}",
                file_path.display()
            ))?;
        
        return Ok(true);
    }

    Ok(false)
}

/// Preview what replacements would be applied (for dry run)
pub fn preview_replacements(target_dir: &str, replacements: &[ReplacementConfig]) -> Result<Vec<ReplacementPreview>> {
    let mut previews = Vec::new();
    let target_path = Path::new(target_dir);

    for replacement in replacements {
        let replacement_value = get_replacement_value(replacement)?;
        let files = find_files_with_pattern(target_path, &replacement.source)?;
        
        previews.push(ReplacementPreview {
            search_pattern: replacement.source.clone(),
            replacement_value,
            affected_files: files,
        });
    }

    Ok(previews)
}

/// Find all files that contain a specific pattern
fn find_files_with_pattern(target_path: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
    let mut matching_files = Vec::new();

    if target_path.is_file() {
        if file_contains_pattern(target_path, pattern)? {
            matching_files.push(target_path.to_path_buf());
        }
    } else if target_path.is_dir() {
        for entry in WalkDir::new(target_path) {
            let entry = entry?;
            
            if entry.file_type().is_file() && file_contains_pattern(entry.path(), pattern)? {
                matching_files.push(entry.path().to_path_buf());
            }
        }
    }

    Ok(matching_files)
}

/// Check if a file contains a specific pattern
fn file_contains_pattern(file_path: &Path, pattern: &str) -> Result<bool> {
    // Skip binary files
    if is_binary_file(file_path)? {
        return Ok(false);
    }

    let content = fs::read_to_string(file_path)
        .with_context(|| format!(
            "Failed to read file for pattern check: {}",
            file_path.display()
        ))?;

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
    use tempfile::TempDir;
    use std::io::Write;

    #[test]
    fn test_apply_simple_replacement() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        
        // Create test file
        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(file, "Hello {{{{NAME}}}}, welcome to {{{{PLACE}}}}!").unwrap();
        drop(file); // Ensure file is flushed and closed

        // Create replacement config
        let replacement = ReplacementConfig {
            source: "{{NAME}}".to_string(),
            target: Some("Alice".to_string()),
            value_from_env: None,
        };

        // Apply replacement
        let result = apply_single_replacement(
            temp_dir.path(),
            &replacement.source,
            &replacement.target.as_ref().unwrap()
        );
        
        assert!(result.is_ok());
        
        // Verify replacement was applied
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("Hello Alice"));
        assert!(content.contains("{{PLACE}}"));
    }

    #[test]
    fn test_replacement_with_env_var() {
        unsafe { std::env::set_var("TEST_ENV", "TestValue"); }

        let replacement = ReplacementConfig {
            source: "{{TEST}}".to_string(),
            target: None,
            value_from_env: Some("TEST_ENV".to_string()),
        };

        let value = get_replacement_value(&replacement);
        assert!(value.is_ok());
        assert_eq!(value.unwrap(), "TestValue");
    }
}
