//! REPL (Read-Eval-Print Loop) for the Ondeks CLI.

use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use ondeks_runtime::Host;
use ondeks_runtime::queue::RuntimeEvent;
use anyhow::Result;

pub struct Repl {
    host: Host,
    editor: DefaultEditor,
    running: bool,
}

impl Repl {
    pub fn new(host: Host) -> Result<Self> {
        let editor = DefaultEditor::new()?;
        Ok(Self {
            host,
            editor,
            running: true,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        // Start audio
        self.host.start()?;

        while self.running {
            match self.editor.readline("> ") {
                Ok(line) => {
                    let line = line.trim();
                    if !line.is_empty() {
                        self.editor.add_history_entry(line)?;
                        if let Err(e) = self.process_line(line) {
                            if e.to_string() == "exit" {
                                break;
                            }
                            println!("Error: {}", e);
                        }
                    }
                }
                Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                    break;
                }
                Err(e) => {
                    println!("Error: {}", e);
                    break;
                }
            }

            // Poll and display events
            for event in self.host.poll_events() {
                self.handle_event(event);
            }
        }

        self.host.stop()?;
        Ok(())
    }

    fn process_line(&mut self, line: &str) -> Result<()> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        match parts[0] {
            "help" => self.cmd_help(),
            "quit" | "exit" => return Err(anyhow::anyhow!("exit")),
            "play" => self.cmd_play(),
            "stop" => self.cmd_stop(),
            "tempo" => self.cmd_tempo(&parts),
            "devices" => self.cmd_devices(),
            "test-tone" => self.cmd_test_tone(&parts),
            _ => {
                println!("Unknown command: {}. Type 'help' for available commands.", parts[0]);
                Ok(())
            }
        }
    }

    fn cmd_help(&self) -> Result<()> {
        println!("Available commands:");
        println!("  help              - Show this help");
        println!("  quit, exit        - Exit the program");
        println!("  play              - Start playback");
        println!("  stop              - Stop playback");
        println!("  tempo <bpm>       - Set tempo");
        println!("  devices           - List audio devices");
        println!("  test-tone [freq] [duration] - Play a test tone");
        Ok(())
    }

    fn cmd_play(&mut self) -> Result<()> {
        use ondeks_core::Command;
        self.host.send_command(Command::Play)?;
        println!("Playing");
        Ok(())
    }

    fn cmd_stop(&mut self) -> Result<()> {
        use ondeks_core::Command;
        self.host.send_command(Command::Stop)?;
        println!("Stopped");
        Ok(())
    }

    fn cmd_tempo(&mut self, parts: &[&str]) -> Result<()> {
        use ondeks_core::Command;
        let bpm: f64 = parts.get(1)
            .ok_or_else(|| anyhow::anyhow!("Usage: tempo <bpm>"))?
            .parse()?;
        self.host.send_command(Command::SetTempo(bpm))?;
        println!("Tempo: {} BPM", bpm);
        Ok(())
    }

    fn cmd_devices(&self) -> Result<()> {
        println!("Audio devices:");
        for device in self.host.list_devices() {
            let marker = if device.is_default { "*" } else { " " };
            println!("  {} {} ({} ch)", marker, device.name, device.max_channels);
        }
        Ok(())
    }

    fn cmd_test_tone(&mut self, parts: &[&str]) -> Result<()> {
        let freq: f32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(440.0);
        let duration: f32 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(1.0);
        
        println!("Playing {} Hz for {} seconds", freq, duration);
        self.host.play_test_tone(freq, duration)?;
        Ok(())
    }

    fn handle_event(&self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::MeterUpdate { .. } => {
                // Could display meters here
            }
            RuntimeEvent::Underrun => {
                println!("Warning: Audio underrun!");
            }
            _ => {}
        }
    }
}
