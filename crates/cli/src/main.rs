//! Ondeks CLI — Command-line interface for the Ondeks DAW engine

use anyhow::Result;
use clap::Parser;
use ondeks_runtime::Host;

mod parser;
mod commands;
mod repl;

#[derive(Parser)]
#[command(name = "ondeks")]
#[command(about = "Digital Audio Workstation Engine", long_about = None)]
#[command(version)]
struct Cli {
    /// Project file to open
    #[arg(value_name = "FILE")]
    project: Option<String>,

    /// Run in non-interactive mode with a command
    #[arg(short = 'c', long = "command")]
    command: Option<String>,

    /// Output format for non-interactive mode
    #[arg(long, default_value = "text")]
    format: String,

    /// Audio device to use
    #[arg(long)]
    device: Option<String>,

    /// Buffer size in samples
    #[arg(long, default_value = "512")]
    buffer_size: usize,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = ondeks_runtime::audio::AudioConfig {
        buffer_size: cli.buffer_size,
        ..Default::default()
    };

    let host = Host::with_config(config)?;

    // Select device if specified
    if let Some(_device) = &cli.device {
        // host.select_device(_device)?;
    }

    // Non-interactive mode
    if let Some(command) = &cli.command {
        // Execute single command and exit
        // Would need to set up context and run command
        println!("Running: {}", command);
        return Ok(());
    }

    // Interactive REPL
    let mut repl = repl::Repl::new(host)?;

    // Load project if specified
    if let Some(project_path) = &cli.project {
        println!("Loading project: {}", project_path);
        // repl.load_project(project_path)?;
    }

    repl.run()
}
