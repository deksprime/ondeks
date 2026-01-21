//! Ondeks CLI — Command-line interface for the Ondeks DAW engine

use anyhow::Result;
use ondeks_runtime::Host;

mod commands;
mod repl;

fn main() -> Result<()> {
    println!("Ondeks DAW Engine v{}", env!("CARGO_PKG_VERSION"));
    println!("Type 'help' for commands, 'quit' to exit.\n");

    let host = Host::new()?;
    let mut repl = repl::Repl::new(host)?;
    
    repl.run()
}
