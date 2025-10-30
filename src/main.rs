//! # `TixGraft`
//!
//! `TixGraft` is a command-line tool for selectively extracting and reusing components from remote Git repositories.
//! It supports sparse checkout, flexible text replacements, and efficient configuration workflows for mono-repos or codebase sharing.
//!
//! ## Features
//! - Fetch only required files or directories from large repositories (sparse checkout).
//! - Apply context-based or manual text replacements on-the-fly.
//! - Generate and consume YAML configuration files for repeatable pulls.
//! - Output equivalent CLI commands from config for transparency and scripting.
//! - Handles complex nested paths and reference formats.
//!
//! ## Usage
//!
//! **Basic example:**  
//! ```sh
//! tixgraft --repository https://github.com/user/repo --pulls source:path/in/repo target:./localdir
//! ```
//!
//! **With config:**  
//! ```sh
//! tixgraft --config tixgraft.yaml
//! ```
//!
//! See `tixgraft --help` or documentation for more options and details.
//!
//! ## Issue Tracking & Contribution
//! - All issues are tracked with `bd (beads)`; see AGENTS.md for workflow guidelines.
//! - Please file bugs, features, or chores using `bd create ...` (never markdown checklists).
//! - All tests use the System abstraction for isolation (see Testing Patterns in AGENTS.md).
//!
//! ---
//! © 2024 `TixGraft` Authors. MIT or Apache-2.0 licensed. See README and LICENSE files for more info.

use anyhow::Result;
use clap::Parser as _;
use tixgraft::cli::Args;
use tixgraft::error::GraftError;
use tixgraft::operations::to_command_line::OutputFormat;
use tixgraft::system::real::RealSystem;
use tracing::error;
use tracing_subscriber::{EnvFilter, fmt};

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing subscriber based on verbose flag
    // Don't use verbose logging for conversion modes
    let log_level = if args.to_command_line || args.to_config {
        "error" // Only show errors for conversion modes
    } else if args.verbose {
        "debug"
    } else {
        "info"
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    fmt().with_target(false).with_env_filter(filter).init();

    // Handle to-config mode
    if args.to_config {
        let system = RealSystem::new();
        match tixgraft::run_to_config(&args, &system) {
            Ok(()) => std::process::exit(0),
            Err(err) => {
                error!("{}", err);
                std::process::exit(
                    err.downcast_ref::<GraftError>()
                        .map_or(1, GraftError::exit_code),
                );
            }
        }
    }

    // Handle to-command-line mode
    if args.to_command_line {
        let format = args
            .output_format
            .parse::<OutputFormat>()
            .unwrap_or_else(|err| {
                error!("{}", err);
                std::process::exit(1);
            });

        match tixgraft::run_to_command_line(
            &args.config,
            format,
            args.repository.clone(),
            args.tag.clone(),
        ) {
            Ok(()) => std::process::exit(0),
            Err(err) => {
                error!("{}", err);
                std::process::exit(
                    err.downcast_ref::<GraftError>()
                        .map_or(1, GraftError::exit_code),
                );
            }
        }
    }

    // Normal execution mode
    match tixgraft::run(args) {
        Ok(()) => std::process::exit(0),
        Err(err) => {
            error!("{}", err);
            std::process::exit(
                err.downcast_ref::<GraftError>()
                    .map_or(1, GraftError::exit_code),
            );
        }
    }
}
