//! Path manipulation and validation helpers.
//!
//! Most read the path text alone; those that canonicalize or probe for a file touch the disk.

use crate::error::GraftError;
use anyhow::{Result, anyhow};
use std::path::{Component, Path, PathBuf};

/// Resolves `.` and `..` from the path text alone: a `..` pops the component before it, even
/// another `..`, and stays only when there is none.
#[must_use]
#[inline]
pub fn normalize(path: &Path) -> PathBuf {
    let mut components = Vec::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if components.is_empty() {
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

/// Rejects an absolute path and one whose normalized form starts with `..`.
///
/// # Errors
///
/// Returns an error if the path is absolute or its normalized form starts with `..`.
#[inline]
pub fn validate_path_safety(path: &str) -> Result<()> {
    let path_obj = Path::new(path);

    if path.contains("..") {
        let normalized = normalize(path_obj);
        let normalized_str = normalized.to_string_lossy();

        if normalized_str.starts_with("..") {
            return Err(GraftError::configuration(format!(
                "Path contains unsafe directory traversal: '{path}' -> '{normalized_str}'"
            ))
            .into());
        }
    }

    if path_obj.is_absolute() && !path.starts_with("./") {
        return Err(GraftError::configuration(format!(
            "Absolute paths are not allowed for security reasons: '{path}'"
        ))
        .into());
    }

    Ok(())
}

/// Canonicalizes both paths, so both must exist, and strips `base` from `path`.
///
/// # Errors
///
/// Returns an error if either path does not exist or `path` is outside `base`.
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

/// Compares canonical paths when both resolve, otherwise the lexically normalized ones.
#[must_use]
#[inline]
pub fn escapes_from_base(path: &Path, base: &Path) -> bool {
    if let Ok(canonical_path) = path.canonicalize()
        && let Ok(canonical_base) = base.canonicalize()
    {
        return !canonical_path.starts_with(&canonical_base);
    }

    let normalized_path = normalize(path);
    let normalized_base = normalize(base);

    !normalized_path.starts_with(&normalized_base)
}

/// Compares whole components, so `/ab` and `/ac` share only `/`.
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

/// Convert backslashes to forward slashes (for cross-platform compatibility).
#[must_use]
#[inline]
pub fn normalize_separators(path: &str) -> String {
    path.replace('\\', "/")
}

/// Joins `component`, once it passes `validate_path_safety`, onto `base` with forward slashes.
///
/// # Errors
///
/// Returns an error if `component` fails the safety check.
#[inline]
pub fn join_path_safe(base: &str, component: &str) -> Result<String> {
    validate_path_safety(component)?;

    let base_path = Path::new(base);
    let result = base_path.join(component);

    Ok(normalize_separators(&result.to_string_lossy()))
}

/// Extract file extension in lowercase.
#[must_use]
#[inline]
pub fn get_file_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_lowercase)
}

/// Check if a path has a specific extension (case-insensitive).
#[must_use]
#[inline]
pub fn has_extension(path: &Path, extension: &str) -> bool {
    get_file_extension(path).is_some_and(|ext| ext == extension.to_lowercase())
}

/// Appends `-1`, `-2` and so on to the stem until the path is free; past 10000 it returns
/// `{stem}-final` unchecked.
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

    let mut counter = 1_i32;
    loop {
        let filename = extension.as_ref().map_or_else(
            || format!("{stem}-{counter}"),
            |ext| format!("{stem}-{counter}.{ext}"),
        );

        let new_path = parent.join(filename);
        if !new_path.exists() {
            return new_path;
        }
        counter = counter.saturating_add(1);

        if counter > 10000 {
            return parent.join(format!("{stem}-final"));
        }
    }
}

/// True when the path contains a pattern as plain text, or matches one whose `*` is a wildcard.
#[must_use]
#[inline]
pub fn is_path_allowed(path: &Path, allowed_patterns: &[&str]) -> bool {
    let path_str = path.to_string_lossy();

    for pattern in allowed_patterns {
        if path_str.contains(pattern) {
            return true;
        }

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

/// Replaces every backslash with a forward slash.
#[inline]
#[must_use]
pub fn to_unix(path: &str) -> String {
    path.replace('\\', "/")
}

/// Replaces every forward slash with a backslash.
#[inline]
#[must_use]
pub fn to_windows(path: &str) -> String {
    path.replace('/', "\\")
}

/// Counts the normal components, ignoring the root, `.` and `..`.
#[inline]
#[must_use]
pub fn depth(path: &Path) -> usize {
    path.components()
        .filter(|component| matches!(component, Component::Normal(_)))
        .count()
}
