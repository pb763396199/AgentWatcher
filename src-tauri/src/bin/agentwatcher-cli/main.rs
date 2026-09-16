mod cli;

use clap::Parser;

fn main() {
    let _parsed = cli::Cli::parse();
}
