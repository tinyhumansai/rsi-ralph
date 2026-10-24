//! The `rsi-ralph` command.

use clap::Parser;
use rsi_ralph_cli::cli::{Cli, execute, exit_code};
use rsi_ralph_cli::error_chain;

fn main() {
    let cli = Cli::parse();
    let result = execute(
        cli,
        &|name| std::env::var(name).ok(),
        &mut std::io::stdout(),
    );
    if let Err(error) = &result {
        eprintln!("rsi-ralph: {}", error_chain(error));
    }
    std::process::exit(exit_code(&result));
}
