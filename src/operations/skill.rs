//! Skill management operations
//!
//! Handles installing, uninstalling, and testing the tixgraft Claude Code skill.
//! The skill content is embedded in the binary at compile time from the `skill/` directory.

use std::collections::HashSet;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use include_dir::{Dir, include_dir};

use crate::error::GraftError;
use crate::system::System;

static SKILL_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/skill");

const SKILL_SUBDIR: &str = ".claude/skills/tixgraft";

/// Result of checking the skill installation status
#[derive(Debug, PartialEq, Eq)]
pub enum SkillStatus {
    /// Skill is not installed at the target location
    NotInstalled,
    /// Skill is installed and matches the embedded content exactly
    UpToDate,
    /// Skill is installed but content differs from the embedded version
    Outdated,
}

/// Resolve the target directory for skill installation
///
/// # Errors
///
/// Returns an error if the home directory or current directory cannot be determined.
pub fn resolve_skill_path(global: bool) -> Result<PathBuf, GraftError> {
    if global {
        let home = dirs::home_dir()
            .ok_or_else(|| GraftError::skill("Could not determine home directory"))?;
        Ok(home.join(SKILL_SUBDIR))
    } else {
        let cwd = std::env::current_dir()
            .map_err(|e| GraftError::skill(format!("Could not determine current directory: {e}")))?;
        Ok(cwd.join(SKILL_SUBDIR))
    }
}

/// Install the embedded skill files to the target directory.
///
/// Creates all necessary directories and writes every embedded file.
/// Overwrites any existing files.
///
/// # Errors
///
/// Returns an error if directory creation or file writing fails.
pub fn skill_install(system: &dyn System, target_dir: &Path) -> Result<(), GraftError> {
    system
        .create_dir_all(target_dir)
        .map_err(|e| GraftError::skill(format!("Failed to create directory: {e}")))?;

    write_embedded_files(system, target_dir, &SKILL_DIR)?;

    eprintln!("Skill installed to {}", target_dir.display());
    Ok(())
}

/// Uninstall the skill by removing the target directory.
///
/// Idempotent: returns Ok even if the directory does not exist.
///
/// # Errors
///
/// Returns an error if the directory exists but cannot be removed.
pub fn skill_uninstall(system: &dyn System, target_dir: &Path) -> Result<(), GraftError> {
    let exists = system
        .exists(target_dir)
        .map_err(|e| GraftError::skill(format!("Failed to check directory: {e}")))?;

    if exists {
        system
            .remove_dir_all(target_dir)
            .map_err(|e| GraftError::skill(format!("Failed to remove directory: {e}")))?;
        eprintln!("Skill uninstalled from {}", target_dir.display());
    } else {
        eprintln!("Skill was not installed at {}", target_dir.display());
    }
    Ok(())
}

/// Check whether the installed skill matches the embedded content exactly.
///
/// Performs a bidirectional comparison:
/// - Every embedded file must exist in the installed directory with identical bytes
/// - No extra files may exist in the installed directory beyond what's embedded
///
/// # Errors
///
/// Returns an error if filesystem operations fail.
pub fn skill_check(system: &dyn System, target_dir: &Path) -> Result<SkillStatus, GraftError> {
    let exists = system
        .exists(target_dir)
        .map_err(|e| GraftError::skill(format!("Failed to check directory: {e}")))?;

    if !exists {
        return Ok(SkillStatus::NotInstalled);
    }

    // Collect all relative paths from the embedded directory
    let mut embedded_paths: HashSet<PathBuf> = HashSet::new();
    collect_embedded_paths(&SKILL_DIR, &mut embedded_paths);

    // Check that every embedded file exists and matches
    if !embedded_files_match(system, target_dir, &SKILL_DIR)? {
        return Ok(SkillStatus::Outdated);
    }

    // Check for extra files in the installed directory
    let installed_paths = collect_installed_paths(system, target_dir, target_dir)?;
    if installed_paths != embedded_paths {
        return Ok(SkillStatus::Outdated);
    }

    Ok(SkillStatus::UpToDate)
}

/// Prompt the user with a yes/no question on stderr/stdin.
///
/// Returns `true` if the user answers "y" or "yes" (case-insensitive).
/// Any other input (including empty / EOF) returns `false`.
///
/// # Errors
///
/// Returns an error if reading from stdin fails.
pub fn prompt_yes_no(question: &str) -> Result<bool, GraftError> {
    eprint!("{question} [y/N] ");
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .map_err(|e| GraftError::skill(format!("Failed to read input: {e}")))?;
    let trimmed = input.trim();
    Ok(trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("yes"))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn write_embedded_files(
    system: &dyn System,
    base: &Path,
    dir: &Dir<'_>,
) -> Result<(), GraftError> {
    for file in dir.files() {
        let target_path = base.join(file.path());
        if let Some(parent) = target_path.parent() {
            system
                .create_dir_all(parent)
                .map_err(|e| GraftError::skill(format!("Failed to create directory: {e}")))?;
        }
        system
            .write(&target_path, file.contents())
            .map_err(|e| {
                GraftError::skill(format!(
                    "Failed to write {}: {e}",
                    target_path.display()
                ))
            })?;
    }

    for sub_dir in dir.dirs() {
        write_embedded_files(system, base, sub_dir)?;
    }

    Ok(())
}

fn collect_embedded_paths(dir: &Dir<'_>, paths: &mut HashSet<PathBuf>) {
    for file in dir.files() {
        paths.insert(file.path().to_path_buf());
    }
    for sub_dir in dir.dirs() {
        collect_embedded_paths(sub_dir, paths);
    }
}

fn embedded_files_match(
    system: &dyn System,
    base: &Path,
    dir: &Dir<'_>,
) -> Result<bool, GraftError> {
    for file in dir.files() {
        let installed_path = base.join(file.path());
        let exists = system
            .exists(&installed_path)
            .map_err(|e| GraftError::skill(format!("Failed to check file: {e}")))?;
        if !exists {
            return Ok(false);
        }

        let mut installed_bytes = Vec::new();
        system
            .open(&installed_path)
            .map_err(|e| {
                GraftError::skill(format!(
                    "Failed to open {}: {e}",
                    installed_path.display()
                ))
            })?
            .read_to_end(&mut installed_bytes)
            .map_err(|e| {
                GraftError::skill(format!(
                    "Failed to read {}: {e}",
                    installed_path.display()
                ))
            })?;

        if installed_bytes != file.contents() {
            return Ok(false);
        }
    }

    for sub_dir in dir.dirs() {
        if !embedded_files_match(system, base, sub_dir)? {
            return Ok(false);
        }
    }

    Ok(true)
}

fn collect_installed_paths(
    system: &dyn System,
    base: &Path,
    current: &Path,
) -> Result<HashSet<PathBuf>, GraftError> {
    let mut paths = HashSet::new();

    let entries = system
        .read_dir(current)
        .map_err(|e| GraftError::skill(format!("Failed to read directory: {e}")))?;

    for entry in entries {
        let is_dir = system
            .is_dir(&entry)
            .map_err(|e| GraftError::skill(format!("Failed to check entry: {e}")))?;

        if is_dir {
            let sub_paths = collect_installed_paths(system, base, &entry)?;
            paths.extend(sub_paths);
        } else {
            let relative = entry
                .strip_prefix(base)
                .map_err(|e| GraftError::skill(format!("Failed to compute relative path: {e}")))?;
            paths.insert(relative.to_path_buf());
        }
    }

    Ok(paths)
}
