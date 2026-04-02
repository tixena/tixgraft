//! `TixGraft` - A CLI tool for fetching reusable components from Git repositories.
//!
//! This library provides functionality to fetch specific files or directories
//! from Git repositories using sparse checkout, apply text replacements, and
//! execute post-processing commands.


pub mod cli;
pub mod config;
pub mod error;
pub mod git;
pub mod operations;
pub mod system;
pub mod utils;

use anyhow::Result;
use cli::Args;
use config::Config;
use operations::pull::PullOperation;
use operations::skill::{self, SkillStatus};
use operations::to_command_line::{OutputFormat, generate_command_line};
use operations::to_config::generate_yaml_config;
use system::System;

use crate::system::real::RealSystem;

/// Main entry point for the tixgraft library.
///
/// # Errors
///
/// Returns an error if:
/// - Configuration loading or validation fails
/// - Git operations fail (clone, sparse checkout)
/// - File operations fail (copy, read, write)
/// - Post-processing commands fail
#[inline]
pub fn run(args: Args) -> Result<()> {
    let system = RealSystem;
    let pull_operation = PullOperation::new(args, &system)?;
    pull_operation.execute()
}

/// Run the to-command-line command.
///
/// # Errors
///
/// Returns an error if:
/// - Configuration file cannot be loaded or parsed
/// - Configuration validation fails
/// - Command line generation fails
#[expect(clippy::print_stdout, reason = "This is a CLI tool")]
#[inline]
pub fn run_to_command_line(
    config_path: &str,
    format: OutputFormat,
    repo_override: Option<String>,
    tag_override: Option<String>,
) -> Result<()> {
    let system = RealSystem;

    // Load config (with overrides if provided)
    let mut config = Config::load_from_file(&system, config_path)?;

    // Apply CLI overrides
    if let Some(repo) = repo_override {
        config.repository = Some(repo);
    }
    if let Some(tag) = tag_override {
        config.tag = Some(tag);
    }

    // Validate merged config
    config.validate(&system)?;

    // Generate command line
    let command_line = generate_command_line(&config, format)?;

    // Output to stdout (not using logging)
    println!("{command_line}");

    Ok(())
}

/// Install the tixgraft Claude Code skill.
///
/// # Errors
///
/// Returns an error if directory creation or file writing fails.
#[inline]
pub fn run_skill_install(global: bool) -> Result<()> {
    let system = RealSystem;
    let target_dir = skill::resolve_skill_path(global)?;
    skill::skill_install(&system, &target_dir)?;
    Ok(())
}

/// Uninstall the tixgraft Claude Code skill.
///
/// # Errors
///
/// Returns an error if the directory exists but cannot be removed.
#[inline]
pub fn run_skill_uninstall(global: bool) -> Result<()> {
    let system = RealSystem;
    let target_dir = skill::resolve_skill_path(global)?;
    skill::skill_uninstall(&system, &target_dir)?;
    Ok(())
}

/// Test whether the tixgraft Claude Code skill is installed and up to date.
///
/// Returns an exit code:
/// - 0: installed and up to date (or user chose to install/upgrade)
/// - 1: not installed and user declined
/// - 2: outdated and user declined upgrade
///
/// # Errors
///
/// Returns an error if filesystem operations fail.
#[inline]
pub fn run_skill_test(global: bool, auto_yes: bool) -> Result<i32> {
    let system = RealSystem;
    let target_dir = skill::resolve_skill_path(global)?;

    match skill::skill_check(&system, &target_dir)? {
        SkillStatus::UpToDate => {
            eprintln!("Skill is installed and up to date.");
            Ok(0)
        }
        SkillStatus::NotInstalled => {
            let should_install =
                auto_yes || skill::prompt_yes_no("Skill is not installed. Install now?")?;
            if should_install {
                skill::skill_install(&system, &target_dir)?;
                Ok(0)
            } else {
                Ok(1)
            }
        }
        SkillStatus::Outdated => {
            let should_upgrade =
                auto_yes || skill::prompt_yes_no("Skill is outdated. Upgrade now?")?;
            if should_upgrade {
                skill::skill_install(&system, &target_dir)?;
                Ok(0)
            } else {
                Ok(2)
            }
        }
    }
}

/// Run the to-config command.
///
/// # Errors
///
/// Returns an error if:
/// - YAML configuration generation fails
/// - Arguments cannot be converted to valid configuration
#[expect(clippy::print_stdout, reason = "This is a CLI tool")]
#[inline]
pub fn run_to_config(args: &Args, system: &dyn System) -> Result<()> {
    // Generate YAML config
    let yaml = generate_yaml_config(args, system)?;

    // Output to stdout (not using logging)
    println!("{yaml}");

    Ok(())
}
