//! Command registry and execution framework.

use crate::parser::ParsedCommand;
use std::collections::HashMap;
use std::sync::Arc;

/// Result of executing a command.
pub type CommandResult = Result<CommandOutput, String>;

/// Output from a command.
#[derive(Debug)]
pub enum CommandOutput {
    /// Simple text output.
    Text(String),
    /// Structured data (for potential JSON output).
    Data(serde_json::Value),
    /// No output needed.
    Silent,
    /// Request to exit the REPL.
    Exit,
}

impl CommandOutput {
    pub fn text(s: impl Into<String>) -> Self {
        Self::Text(s.into())
    }

    pub fn silent() -> Self {
        Self::Silent
    }
}

/// Metadata about a command.
#[derive(Clone)]
pub struct CommandInfo {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub usage: &'static str,
    pub description: &'static str,
    pub examples: &'static [&'static str],
    pub category: CommandCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    Project,
    Transport,
    Track,
    Clip,
    Session,
    Mixer,
    Plugin,
    System,
}

impl CommandCategory {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Project => "Project",
            Self::Transport => "Transport",
            Self::Track => "Track",
            Self::Clip => "Clip",
            Self::Session => "Session",
            Self::Mixer => "Mixer",
            Self::Plugin => "Plugin",
            Self::System => "System",
        }
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Project,
            Self::Transport,
            Self::Track,
            Self::Clip,
            Self::Session,
            Self::Mixer,
            Self::Plugin,
            Self::System,
        ]
    }
}

/// Handler function type.
pub type CommandHandler = Arc<dyn Fn(&mut CommandContext, &ParsedCommand) -> CommandResult + Send + Sync>;

/// Context passed to command handlers.
pub struct CommandContext<'a> {
    pub host: &'a mut ondeks_runtime::Host,
    pub project: &'a mut ondeks_core::project::Project,
    pub view_manager: &'a mut ondeks_core::session::ViewManager,
}

/// Registry of all commands.
pub struct CommandRegistry {
    commands: HashMap<String, (CommandInfo, CommandHandler)>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            commands: HashMap::new(),
        };
        registry.register_all();
        registry
    }

    /// Register a command.
    pub fn register(&mut self, info: CommandInfo, handler: CommandHandler) {
        self.commands.insert(info.name.to_string(), (info.clone(), handler.clone()));
        for alias in info.aliases {
            self.commands.insert(alias.to_string(), (info.clone(), handler.clone()));
        }
    }

    /// Execute a command.
    pub fn execute(&self, ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
        match self.commands.get(&cmd.name) {
            Some((_, handler)) => handler(ctx, cmd),
            None => Err(format!("Unknown command: '{}'. Type 'help' for available commands.", cmd.name)),
        }
    }

    /// Get command info.
    pub fn get_info(&self, name: &str) -> Option<&CommandInfo> {
        self.commands.get(name).map(|(info, _)| info)
    }

    /// Get all commands in a category.
    pub fn commands_in_category(&self, category: CommandCategory) -> Vec<&CommandInfo> {
        let mut seen = std::collections::HashSet::new();
        self.commands
            .values()
            .filter(|(info, _)| info.category == category)
            .filter(|(info, _)| seen.insert(info.name))
            .map(|(info, _)| info)
            .collect()
    }

    /// Get all command names for tab completion.
    pub fn all_command_names(&self) -> Vec<&str> {
        let mut seen = std::collections::HashSet::new();
        self.commands
            .values()
            .filter(|(info, _)| seen.insert(info.name))
            .map(|(info, _)| info.name)
            .collect()
    }

    fn register_all(&mut self) {
        // Register all command modules
        super::project::register(self);
        super::transport::register(self);
        super::track::register(self);
        super::session::register(self);
        super::system::register(self);
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}
