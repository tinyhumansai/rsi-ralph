//! The `tinyrsi` command.

use clap::Parser;
use tinyrsi_cli::cli::{Cli, execute, exit_code};
use tinyrsi_cli::error_chain;

fn main() {
    let cli = Cli::parse();
    let result = execute(
        cli,
        &|name| std::env::var(name).ok(),
        &mut std::io::stdout(),
    );
    if let Err(error) = &result {
        eprintln!("tinyrsi: {}", error_chain(error));
    }
    std::process::exit(exit_code(&result));
}
