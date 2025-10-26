use anyhow::Result;
use clap::Parser as _;
use tixgraft::cli::Args;
use tixgraft::error::GraftError;
use tixgraft::operations::to_command_line::OutputFormat;
use tracing::error;
use tracing_subscriber::{EnvFilter, fmt};

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing subscriber based on verbose flag
    // Don't use verbose logging for to-command-line mode
    let log_level = if args.to_command_line {
        "error" // Only show errors for to-command-line
    } else if args.verbose {
        "debug"
    } else {
        "info"
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    fmt().with_target(false).with_env_filter(filter).init();

    // Handle to-command-line mode
    if args.to_command_line {
        let format = args
            .output_format
            .parse::<OutputFormat>()
            .unwrap_or_else(|e| {
                error!("{}", e);
                std::process::exit(1);
            });

        match tixgraft::run_to_command_line(
            &args.config,
            format,
            args.repository.clone(),
            args.tag.clone(),
        ) {
            Ok(()) => std::process::exit(0),
            Err(e) => {
                error!("{}", e);
                std::process::exit(
                    e.downcast_ref::<GraftError>()
                        .map_or(1, |ge| ge.exit_code()),
                );
            }
        }
    }

    // Normal execution mode
    match tixgraft::run(args) {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            error!("{}", e);
            std::process::exit(
                e.downcast_ref::<GraftError>()
                    .map_or(1, |ge| ge.exit_code()),
            );
        }
    }
}
