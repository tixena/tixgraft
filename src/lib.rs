//! `TixGraft` - A CLI tool for fetching reusable components from Git repositories
//!
//! This library provides functionality to fetch specific files or directories
//! from Git repositories using sparse checkout, apply text replacements, and
//! execute post-processing commands.

#![expect(clippy::allow_attributes_without_reason)]
#![expect(clippy::pub_use)]
#![expect(clippy::single_call_fn)]
#![expect(clippy::question_mark_used)]
#![expect(clippy::arithmetic_side_effects)]
#![expect(clippy::option_if_let_else)]
#![expect(clippy::min_ident_chars)]
#![expect(clippy::float_arithmetic)]
#![expect(clippy::iter_over_hash_type)]
#![expect(clippy::needless_pass_by_value)]
#![expect(clippy::module_name_repetitions)]
#![expect(clippy::mod_module_files)]
#![expect(clippy::ref_patterns)]
#![allow(clippy::missing_docs_in_private_items)]

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
use operations::to_command_line::{OutputFormat, generate_command_line};
use operations::to_config::generate_yaml_config;
use system::System;

use crate::system::real::RealSystem;

/// Main entry point for the tixgraft library
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

/// Run the to-command-line command
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

/// Run the to-config command
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
