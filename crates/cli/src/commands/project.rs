//! Project management commands.

use super::registry::*;
use crate::parser::ParsedCommand;
use std::sync::Arc;

pub fn register(registry: &mut CommandRegistry) {
    // new
    registry.register(
        CommandInfo {
            name: "new",
            aliases: &[],
            usage: "new <name>",
            description: "Create a new project",
            examples: &["new \"My Song\"", "new Untitled"],
            category: CommandCategory::Project,
        },
        Arc::new(cmd_new),
    );

    // load
    registry.register(
        CommandInfo {
            name: "load",
            aliases: &["open"],
            usage: "load <path>",
            description: "Load a project from file",
            examples: &["load mysong.ondeks", "load ~/projects/demo.ondeks"],
            category: CommandCategory::Project,
        },
        Arc::new(cmd_load),
    );

    // save
    registry.register(
        CommandInfo {
            name: "save",
            aliases: &[],
            usage: "save [path]",
            description: "Save the current project",
            examples: &["save", "save mysong.ondeks"],
            category: CommandCategory::Project,
        },
        Arc::new(cmd_save),
    );

    // export
    registry.register(
        CommandInfo {
            name: "export",
            aliases: &["bounce"],
            usage: "export <path> [--start <beats>] [--end <beats>] [--normalize]",
            description: "Export project to audio file",
            examples: &[
                "export output.wav",
                "export mix.wav --start 0 --end 64",
                "export final.wav --normalize",
            ],
            category: CommandCategory::Project,
        },
        Arc::new(cmd_export),
    );

    // info
    registry.register(
        CommandInfo {
            name: "info",
            aliases: &["project"],
            usage: "info",
            description: "Show project information",
            examples: &["info"],
            category: CommandCategory::Project,
        },
        Arc::new(cmd_info),
    );
}

fn cmd_new(ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    let name = cmd.arg(0).unwrap_or("Untitled");
    *ctx.project = ondeks_core::project::Project::new(name);
    Ok(CommandOutput::text(format!("Created new project: {}", name)))
}

fn cmd_load(ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    let path = cmd.require_arg(0, "path")?;
    
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    
    let project_file: ondeks_core::persistence::ProjectFile = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse project: {}", e))?;
    
    // Convert ProjectFile to Project (simplified)
    ctx.project.meta.name = project_file.meta.name;
    
    Ok(CommandOutput::text(format!("Loaded project from: {}", path)))
}

fn cmd_save(ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    let path = cmd.arg(0).unwrap_or("project.ondeks");
    
    let json = ondeks_core::persistence::project_to_json(ctx.project)
        .map_err(|e| format!("Failed to serialize: {}", e))?;
    
    std::fs::write(path, json)
        .map_err(|e| format!("Failed to write file: {}", e))?;
    
    Ok(CommandOutput::text(format!("Saved project to: {}", path)))
}

fn cmd_export(_ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    let path = cmd.require_arg(0, "path")?;
    
    let _start: f64 = cmd.flag("start").and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let _end: f64 = cmd.flag("end").and_then(|s| s.parse().ok()).unwrap_or(16.0);
    let _normalize = cmd.has_flag("normalize");
    
    // TODO: Implement export using ondeks_runtime::render module
    Ok(CommandOutput::text(format!(
        "Export to {} (not yet implemented)",
        path
    )))
}

fn cmd_info(ctx: &mut CommandContext, _cmd: &ParsedCommand) -> CommandResult {
    let p = &ctx.project;
    let track_count = p.tracks().len();
    let clip_count = p.clips().count();
    
    Ok(CommandOutput::text(format!(
        "Project: {}\n\
         Tempo: {:.1} BPM\n\
         Time Signature: {}\n\
         Tracks: {}\n\
         Clips: {}",
        p.meta.name,
        p.tempo_map.default_tempo(),
        p.time_signature,
        track_count,
        clip_count,
    )))
}
