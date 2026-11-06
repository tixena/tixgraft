//! `TixGraft` - A CLI tool for fetching reusable components from Git
//! repositories or local filesystem paths.
//!
//! This library fetches specific files or directories from a Git repository or a local folder,
//! applies text replacements and runs post-processing commands.

pub mod cli;
pub mod config;
pub mod error;
pub mod git;
pub mod operations;
pub mod utils;

use anyhow::Result;
use cli::Args;
use config::Config;
use operations::pull::PullOperation;
use operations::skill::{self, SkillStatus};
use operations::to_command_line::{OutputFormat, generate_command_line};
use operations::to_config::generate_yaml_config;
use os_shim::System;
use os_shim::real::RealSystem;

/// Runs the pulls the arguments and their configuration file describe, on the real filesystem.
///
/// # Errors
///
/// Returns an error if the configuration is invalid or a Git, file or command step fails.
#[inline]
pub fn run(args: Args) -> Result<()> {
    let system = RealSystem::new();
    let pull_operation = PullOperation::new(args, &system)?;
    pull_operation.execute()
}

/// Prints the file at `config_path` as a command line, with the overrides applied; the output
/// leaves out children and context values.
///
/// # Errors
///
/// Returns an error if the file cannot be loaded, fails validation or cannot be converted.
#[expect(clippy::print_stdout, reason = "This is a CLI tool")]
#[inline]
pub fn run_to_command_line(
    config_path: &str,
    format: OutputFormat,
    repo_override: Option<String>,
    tag_override: Option<String>,
) -> Result<()> {
    let system = RealSystem::new();

    let mut config = Config::load_from_file(&system, config_path)?;

    if let Some(repo) = repo_override {
        config.repository = Some(repo);
    }
    if let Some(tag) = tag_override {
        config.tag = Some(tag);
    }

    config.validate(&system)?;

    let command_line = generate_command_line(&config, format)?;

    println!("{command_line}");

    Ok(())
}

/// Installs the embedded skill under the home directory when `global`, else the current directory.
///
/// # Errors
///
/// Returns an error if the directory cannot be resolved or a file cannot be written.
#[inline]
pub fn run_skill_install(global: bool) -> Result<()> {
    let system = RealSystem::new();
    let target_dir = skill::resolve_skill_path(global)?;
    skill::skill_install(&system, &target_dir)?;
    Ok(())
}

/// Removes the skill under the home directory when `global`, else the current directory; a skill
/// that is not installed is not an error.
///
/// # Errors
///
/// Returns an error if the directory exists but cannot be removed.
#[inline]
pub fn run_skill_uninstall(global: bool) -> Result<()> {
    let system = RealSystem::new();
    let target_dir = skill::resolve_skill_path(global)?;
    skill::skill_uninstall(&system, &target_dir)?;
    Ok(())
}

/// Compares the installed skill with the embedded one and offers to install or upgrade it, which
/// `auto_yes` accepts without asking.
///
/// Returns an exit code:
/// - 0: installed and up to date (or user chose to install/upgrade)
/// - 1: not installed and user declined
/// - 2: outdated and user declined upgrade
///
/// # Errors
///
/// Returns an error if a filesystem operation or the prompt fails.
#[inline]
pub fn run_skill_test(global: bool, auto_yes: bool) -> Result<i32> {
    let system = RealSystem::new();
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

/// Prints the YAML configuration `args` describe; with no config file, their context values are
/// left out.
///
/// # Errors
///
/// Returns an error if `args` do not form a valid configuration or YAML generation fails.
#[expect(clippy::print_stdout, reason = "This is a CLI tool")]
#[inline]
pub fn run_to_config(args: &Args, system: &dyn System) -> Result<()> {
    let yaml = generate_yaml_config(args, system)?;

    println!("{yaml}");

    Ok(())
}
