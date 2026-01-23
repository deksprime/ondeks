//! REPL (Read-Eval-Print Loop) for the Ondeks CLI.

use rustyline::error::ReadlineError;
use rustyline::{Editor, Config, EditMode, CompletionType};
use rustyline::history::DefaultHistory;

use crate::parser::parse_command;
use crate::commands::{CommandRegistry, CommandContext, CommandOutput};
use ondeks_runtime::Host;
use ondeks_core::project::Project;
use ondeks_core::session::ViewManager;

/// Tab completion helper.
struct CommandCompleter {
    commands: Vec<String>,
}

impl CommandCompleter {
    fn new(registry: &CommandRegistry) -> Self {
        Self {
            commands: registry.all_command_names().iter().map(|s| s.to_string()).collect(),
        }
    }
}

impl rustyline::completion::Completer for CommandCompleter {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<String>)> {
        let start = line[..pos].rfind(' ').map(|i| i + 1).unwrap_or(0);
        let word = &line[start..pos];
        
        let matches: Vec<String> = self.commands
            .iter()
            .filter(|cmd| cmd.starts_with(word))
            .cloned()
            .collect();
        
        Ok((start, matches))
    }
}

impl rustyline::highlight::Highlighter for CommandCompleter {}
impl rustyline::hint::Hinter for CommandCompleter {
    type Hint = String;
}
impl rustyline::validate::Validator for CommandCompleter {}
impl rustyline::Helper for CommandCompleter {}

/// The main REPL.
pub struct Repl {
    host: Host,
    project: Project,
    view_manager: ViewManager,
    registry: CommandRegistry,
    editor: Editor<CommandCompleter, DefaultHistory>,
    history_path: Option<std::path::PathBuf>,
}

impl Repl {
    pub fn new(host: Host) -> anyhow::Result<Self> {
        let project = Project::new("Untitled");
        let view_manager = ViewManager::new();
        let registry = CommandRegistry::new();
        
        let config = Config::builder()
            .history_ignore_space(true)
            .completion_type(CompletionType::List)
            .edit_mode(EditMode::Emacs)
            .build();
        
        let completer = CommandCompleter::new(&registry);
        let mut editor = Editor::with_config(config)?;
        editor.set_helper(Some(completer));
        
        // Load history
        let history_path = dirs::data_dir()
            .map(|p| p.join("ondeks").join("history.txt"));
        
        if let Some(ref path) = history_path {
            if path.exists() {
                let _ = editor.load_history(path);
            }
        }
        
        Ok(Self {
            host,
            project,
            view_manager,
            registry,
            editor,
            history_path,
        })
    }

    /// Run the REPL.
    pub fn run(&mut self) -> anyhow::Result<()> {
        // Start audio
        self.host.start()?;
        
        self.print_welcome();
        
        loop {
            let prompt = self.make_prompt();
            
            match self.editor.readline(&prompt) {
                Ok(line) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    
                    self.editor.add_history_entry(line)?;
                    
                    match self.execute_line(line) {
                        Ok(CommandOutput::Text(text)) => println!("{}", text),
                        Ok(CommandOutput::Data(json)) => {
                            println!("{}", serde_json::to_string_pretty(&json).unwrap())
                        }
                        Ok(CommandOutput::Silent) => {}
                        Ok(CommandOutput::Exit) => break,
                        Err(e) => eprintln!("Error: {}", e),
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    println!("^C");
                    // Don't exit, just cancel current input
                }
                Err(ReadlineError::Eof) => {
                    println!("^D");
                    break;
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    break;
                }
            }
            
            // Poll host events
            self.process_events();
        }
        
        // Save history
        if let Some(ref path) = self.history_path {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let _ = self.editor.save_history(path);
        }
        
        // Stop audio
        self.host.stop()?;
        
        println!("Goodbye!");
        Ok(())
    }

    fn print_welcome(&self) {
        println!(
            r#"
    ┌──────────────────────────────────────────────────────────┐
    │                                                          │
    │   ██████╗ ███╗   ██╗██████╗ ███████╗██╗  ██╗███████╗   │
    │  ██╔═══██╗████╗  ██║██╔══██╗██╔════╝██║ ██╔╝██╔════╝   │
    │  ██║   ██║██╔██╗ ██║██║  ██║█████╗  █████╔╝ ███████╗   │
    │  ██║   ██║██║╚██╗██║██║  ██║██╔══╝  ██╔═██╗ ╚════██║   │
    │  ╚██████╔╝██║ ╚████║██████╔╝███████╗██║  ██╗███████║   │
    │   ╚═════╝ ╚═╝  ╚═══╝╚═════╝ ╚══════╝╚═╝  ╚═╝╚══════╝   │
    │                                                          │
    │   Digital Audio Workstation Engine  v{}              │
    │                                                          │
    └──────────────────────────────────────────────────────────┘
    
Type 'help' for available commands.
"#,
            env!("CARGO_PKG_VERSION")
        );
    }

    fn make_prompt(&self) -> String {
        let mode = match self.view_manager.mode {
            ondeks_core::session::ViewMode::Session => "S",
            ondeks_core::session::ViewMode::Arrangement => "A",
        };
        let playing = if self.host.is_running() { "▶" } else { "⏹" };
        
        format!(
            "\x1b[36m{}\x1b[0m [{}|{}] > ",
            self.project.meta.name,
            mode,
            playing
        )
    }

    fn execute_line(&mut self, line: &str) -> Result<CommandOutput, String> {
        let cmd = match parse_command(line) {
            Some(c) => c,
            None => return Ok(CommandOutput::Silent),
        };
        
        let mut ctx = CommandContext {
            host: &mut self.host,
            project: &mut self.project,
            view_manager: &mut self.view_manager,
        };
        
        self.registry.execute(&mut ctx, &cmd)
    }

    fn process_events(&mut self) {
        for event in self.host.poll_events() {
            use ondeks_runtime::queue::RuntimeEvent;
            match event {
                RuntimeEvent::Underrun => {
                    eprintln!("\x1b[33mWarning: Audio underrun!\x1b[0m");
                }
                RuntimeEvent::TransportStateChanged { is_playing: _ } => {
                    // Could update prompt
                }
                _ => {}
            }
        }
    }
}
