//! Binary entry point of tixgraft.
//!
//! Parses the arguments, sets the log level, runs the mode the flags select and exits with the
//! code of the resulting error.

use anyhow::Result;
use clap::Parser as _;
use os_shim::real::RealSystem;
use std::process::exit;
use tixgraft::cli::Args;
use tixgraft::error::GraftError;
use tixgraft::operations::to_command_line::OutputFormat;
use tracing::error;
use tracing_subscriber::{EnvFilter, fmt};

/// Reads the exit code off a `GraftError`; any other error exits with 1.
fn error_to_exit_code(err: &anyhow::Error) -> i32 {
    err.downcast_ref::<GraftError>()
        .map_or(1_i32, GraftError::exit_code)
}

/// Returns 0 for `Ok`; logs an `Err` with its full cause chain and returns its exit code.
fn result_to_exit_code(result: Result<()>) -> i32 {
    match result {
        Ok(()) => 0_i32,
        Err(err) => {
            error!("{:#}", err);
            error_to_exit_code(&err)
        }
    }
}

/// Takes the log level from `RUST_LOG`, or else from the mode: `error` for the conversion and skill
/// modes, `debug` with `--verbose`, otherwise `info`.
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

    let any_skill = args.skill.skill_install || args.skill.skill_uninstall || args.skill.skill_test;
    if args.skill.global && !any_skill {
        error!("--global (-g) requires one of --skill-install, --skill-uninstall, or --skill-test");
        exit(1_i32);
    }
    if args.skill.yes && !args.skill.skill_test {
        error!("--yes (-y) can only be used with --skill-test");
        exit(1_i32);
    }

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

    if args.to_config {
        let system = RealSystem::new();
        exit(result_to_exit_code(tixgraft::run_to_config(&args, &system)));
    }

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

    exit(result_to_exit_code(tixgraft::run(args)));
}
