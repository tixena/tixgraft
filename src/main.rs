use anyhow::Result;
use clap::Parser;
use tixgraft::cli::Args;
use tixgraft::error::GraftError;

fn main() -> Result<()> {
    let args = Args::parse();
    
    match tixgraft::run(args) {
        Ok(_) => std::process::exit(0),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(e.downcast_ref::<GraftError>()
                .map(|ge| ge.exit_code())
                .unwrap_or(1));
        }
    }
}
