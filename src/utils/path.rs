//! Path manipulation and validation utilities

use crate::error::GraftError;
use anyhow::{Result, anyhow};
use std::path::{Component, Path, PathBuf};

/// Normalize a path by resolving `.` and `..` components
#[must_use]
#[inline]
pub fn normalize(path: &Path) -> PathBuf {
    let mut components = Vec::new();

    for component in path.components() {
        match component {
            Component::CurDir => {
                // Skip '.' components
            }
            Component::ParentDir => {
                // Handle '..' by popping the last component if possible
                if components.is_empty() {
                    // Keep leading '..' components
                    components.push(component);
                } else {
                    components.pop();
                }
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                components.push(component);
            }
        }
    }

    components.iter().collect()
}

/// Validate that a path is safe (no directory traversal)
///
/// # Errors
///
/// Returns an error if:
/// - The path contains unsafe directory traversal
/// - The path is an absolute path
#[inline]
pub fn validate_path_safety(path: &str) -> Result<()> {
    let path_obj = Path::new(path);

    // Check for dangerous patterns
    if path.contains("..") {
        // Allow relative paths that don't escape the current directory
        let normalized = normalize(path_obj);
        let normalized_str = normalized.to_string_lossy();

        // If the normalized path starts with ".." then it escapes
        if normalized_str.starts_with("..") {
            return Err(GraftError::configuration(format!(
                "Path contains unsafe directory traversal: '{path}' -> '{normalized_str}'"
            ))
            .into());
        }
    }

    // Disallow absolute paths on Unix-like systems (security risk)
    if path_obj.is_absolute() && !path.starts_with("./") {
        return Err(GraftError::configuration(format!(
            "Absolute paths are not allowed for security reasons: '{path}'"
        ))
        .into());
    }

    Ok(())
}

/// Convert a path to be relative to a base directory
///
/// # Errors
///
/// Returns an error if:
/// - The path does not exist
/// - The base path does not exist
/// - The path is not within the base directory
#[inline]
pub fn make_relative_to(path: &Path, base: &Path) -> Result<PathBuf> {
    let canonical_path = path
        .canonicalize()
        .map_err(|err| anyhow!("Path does not exist: {}. Error: {err}", path.display()))?;
    let canonical_base = base
        .canonicalize()
        .map_err(|err| anyhow!("Base path does not exist: {}. Error: {err}", base.display()))?;

    canonical_path
        .strip_prefix(&canonical_base)
        .map(Path::to_path_buf)
        .map_err(|err| {
            anyhow!(
                "Path '{}' is not within base directory '{}'. Error: {err}",
                path.display(),
                base.display()
            )
        })
}

/// Check if a path would escape the given base directory
#[must_use]
#[inline]
pub fn escapes_from_base(path: &Path, base: &Path) -> bool {
    if let Ok(canonical_path) = path.canonicalize()
        && let Ok(canonical_base) = base.canonicalize()
    {
        return !canonical_path.starts_with(&canonical_base);
    }

    // If we can't canonicalize, check using normalized paths
    let normalized_path = normalize(path);
    let normalized_base = normalize(base);

    !normalized_path.starts_with(&normalized_base)
}

/// Get the common prefix of two paths
#[must_use]
#[inline]
pub fn common_path_prefix(path1: &Path, path2: &Path) -> PathBuf {
    let components1: Vec<_> = path1.components().collect();
    let components2: Vec<_> = path2.components().collect();

    let mut common = PathBuf::new();

    for (c1, c2) in components1.iter().zip(components2.iter()) {
        if c1 == c2 {
            common.push(c1);
        } else {
            break;
        }
    }

    common
}

/// Convert backslashes to forward slashes (for cross-platform compatibility)
#[must_use]
#[inline]
pub fn normalize_separators(path: &str) -> String {
    path.replace('\\', "/")
}

/// Join path components safely
///
/// # Errors
///
/// Returns an error if:
/// - The path is not safe
#[inline]
pub fn join_path_safe(base: &str, component: &str) -> Result<String> {
    validate_path_safety(component)?;

    let base_path = Path::new(base);
    let result = base_path.join(component);

    Ok(normalize_separators(&result.to_string_lossy()))
}

/// Extract file extension in lowercase
#[must_use]
#[inline]
pub fn get_file_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_lowercase)
}

/// Check if a path has a specific extension (case-insensitive)
#[must_use]
#[inline]
pub fn has_extension(path: &Path, extension: &str) -> bool {
    get_file_extension(path).is_some_and(|ext| ext == extension.to_lowercase())
}

/// Get a unique filename by appending a number if the file exists
#[must_use]
#[inline]
pub fn get_unique_filename(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }

    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("file")
        .to_owned();
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(ToOwned::to_owned);
    let parent = path.parent().unwrap_or_else(|| Path::new("")).to_path_buf();

    let mut counter = 1;
    loop {
        let filename = if let Some(ext) = extension.as_ref() {
            format!("{stem}-{counter}.{ext}")
        } else {
            format!("{stem}-{counter}")
        };

        let new_path = parent.join(filename);
        if !new_path.exists() {
            return new_path;
        }
        counter += 1;

        // Prevent infinite loops
        if counter > 10000 {
            return parent.join(format!("{stem}-final"));
        }
    }
}

/// Check if a path is within allowed patterns
#[must_use]
#[inline]
pub fn is_path_allowed(path: &Path, allowed_patterns: &[&str]) -> bool {
    let path_str = path.to_string_lossy();

    for pattern in allowed_patterns {
        if path_str.contains(pattern) {
            return true;
        }

        // Support simple glob patterns
        if pattern.contains('*') {
            let regex_pattern = pattern.replace('*', ".*");
            if let Ok(regex) = regex::Regex::new(&regex_pattern)
                && regex.is_match(&path_str)
            {
                return true;
            }
        }
    }

    false
}

/// Convert a Windows path to Unix-style path
#[inline]
#[must_use]
pub fn to_unix(path: &str) -> String {
    path.replace('\\', "/")
}

/// Convert a Unix path to Windows-style path (for Windows compatibility)
#[inline]
#[must_use]
pub fn to_windows(path: &str) -> String {
    path.replace('/', "\\")
}

/// Get the depth of a path (number of directory levels)
#[inline]
#[must_use]
pub fn depth(path: &Path) -> usize {
    path.components()
        .filter(|component| matches!(component, Component::Normal(_)))
        .count()
}
