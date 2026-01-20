//! Ondeks CLI — Command-line interface for the Ondeks DAW engine

use anyhow::Result;

mod commands;
mod repl;

fn main() -> Result<()> {
    println!("Ondeks DAW Engine v{}", env!("CARGO_PKG_VERSION"));
    println!("Type 'help' for available commands, 'quit' to exit.");

    // TODO: Initialize runtime and start REPL
    Ok(())
}
