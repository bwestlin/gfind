mod cli;
mod commands;
mod config;
mod error;
mod git;
mod logger;
mod query;
mod style;

use crate::cli::Cli;

fn main() {
    let cli = Cli::parse();

    if let Err(err) = commands::run(cli) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
