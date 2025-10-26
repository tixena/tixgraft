use anyhow::Result;
use clap::Parser as _;
use tixgraft::cli::Args;
use tixgraft::error::GraftError;
use tracing::error;
use tracing_subscriber::{EnvFilter, fmt};

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing subscriber based on verbose flag
    let log_level = if args.verbose { "debug" } else { "info" };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    fmt().with_target(false).with_env_filter(filter).init();

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
