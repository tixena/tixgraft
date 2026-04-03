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
use std::process::exit;
use tixgraft::cli::Args;
use tixgraft::error::GraftError;
use tixgraft::operations::to_command_line::OutputFormat;
use tixgraft::system::real::RealSystem;
use tracing::error;
use tracing_subscriber::{EnvFilter, fmt};

/// Map a Result to an exit code: 0 for Ok, or the appropriate error code.
fn error_to_exit_code(err: &anyhow::Error) -> i32 {
    err.downcast_ref::<GraftError>()
        .map_or(1_i32, GraftError::exit_code)
}

/// Convert a Result into an exit code, logging on error.
fn result_to_exit_code(result: Result<()>) -> i32 {
    match result {
        Ok(()) => 0_i32,
        Err(err) => {
            error!("{:#}", err);
            error_to_exit_code(&err)
        }
    }
}

/// Initialize the tracing subscriber with the appropriate log level.
fn init_tracing(args: &Args) {
    let is_skill_mode =
        args.skill.skill_install || args.skill.skill_uninstall || args.skill.skill_test;
    let log_level = if args.to_command_line || args.to_config || is_skill_mode {
        "error"
    } else if args.verbose {
        "debug"
    } else {
        "info"
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));
    fmt().with_target(false).with_env_filter(filter).init();
}

fn main() -> Result<()> {
    let args = Args::parse();
    init_tracing(&args);

    // Validate skill flag constraints
    let any_skill = args.skill.skill_install || args.skill.skill_uninstall || args.skill.skill_test;
    if args.skill.global && !any_skill {
        error!("--global (-g) requires one of --skill-install, --skill-uninstall, or --skill-test");
        exit(1_i32);
    }
    if args.skill.yes && !args.skill.skill_test {
        error!("--yes (-y) can only be used with --skill-test");
        exit(1_i32);
    }

    // Handle skill commands
    if args.skill.skill_install {
        exit(result_to_exit_code(tixgraft::run_skill_install(
            args.skill.global,
        )));
    }
    if args.skill.skill_uninstall {
        exit(result_to_exit_code(tixgraft::run_skill_uninstall(
            args.skill.global,
        )));
    }
    if args.skill.skill_test {
        match tixgraft::run_skill_test(args.skill.global, args.skill.yes) {
            Ok(code) => exit(code),
            Err(err) => {
                error!("{}", err);
                exit(error_to_exit_code(&err));
            }
        }
    }

    // Handle to-config mode
    if args.to_config {
        let system = RealSystem::new();
        exit(result_to_exit_code(tixgraft::run_to_config(&args, &system)));
    }

    // Handle to-command-line mode
    if args.to_command_line {
        let format = args
            .output_format
            .parse::<OutputFormat>()
            .unwrap_or_else(|err| {
                error!("{}", err);
                exit(1_i32);
            });
        let result = tixgraft::run_to_command_line(
            &args.config,
            format,
            args.repository.clone(),
            args.tag.clone(),
        );
        exit(result_to_exit_code(result));
    }

    // Normal execution mode
    exit(result_to_exit_code(tixgraft::run(args)));
}
