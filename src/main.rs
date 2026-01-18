//! Hollowcheck CLI entry point.

use clap::Parser;
use hollowcheck::cli::{self, Cli, Commands, EXIT_ERROR};

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Commands::Lint(args) => match cli::run_lint(&args) {
            Ok(code) => code,
            Err(e) => {
                eprintln!("Error: {}", e);
                EXIT_ERROR
            }
        },
        Commands::Init(args) => match cli::run_init(&args) {
            Ok(code) => code,
            Err(e) => {
                eprintln!("Error: {}", e);
                EXIT_ERROR
            }
        },
    };

    std::process::exit(exit_code);
}
