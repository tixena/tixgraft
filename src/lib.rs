//! TixGraft - A CLI tool for fetching reusable components from Git repositories
//! 
//! This library provides functionality to fetch specific files or directories
//! from Git repositories using sparse checkout, apply text replacements, and
//! execute post-processing commands.

pub mod cli;
pub mod config;
pub mod git;
pub mod operations;
pub mod error;
pub mod utils;

use anyhow::Result;
use cli::Args;
use operations::pull::PullOperation;

/// Main entry point for the tixgraft library
pub fn run(args: Args) -> Result<()> {
    let pull_operation = PullOperation::new(args)?;
    pull_operation.execute()
}
