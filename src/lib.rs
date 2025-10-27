//! `TixGraft` - A CLI tool for fetching reusable components from Git repositories
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
use operations::to_command_line::{OutputFormat, config_to_command_line};
use system::RealSystem;

/// Main entry point for the tixgraft library
pub fn run(args: Args) -> Result<()> {
    let system = RealSystem;
    let pull_operation = PullOperation::new(args, &system)?;
    pull_operation.execute()
}

/// Run the to-command-line command
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
    let command_line = config_to_command_line(&config, format)?;

    // Output to stdout (not using logging)
    println!("{}", command_line);

    Ok(())
}
