//! Transport control commands.

use super::registry::*;
use crate::parser::ParsedCommand;
use ondeks_core::Command;
use std::sync::Arc;

pub fn register(registry: &mut CommandRegistry) {
    registry.register(
        CommandInfo {
            name: "play",
            aliases: &["p"],
            usage: "play",
            description: "Start playback",
            examples: &["play"],
            category: CommandCategory::Transport,
        },
        Arc::new(cmd_play),
    );

    registry.register(
        CommandInfo {
            name: "stop",
            aliases: &["s"],
            usage: "stop",
            description: "Stop playback and return to start",
            examples: &["stop"],
            category: CommandCategory::Transport,
        },
        Arc::new(cmd_stop),
    );

    registry.register(
        CommandInfo {
            name: "pause",
            aliases: &[],
            usage: "pause",
            description: "Pause playback at current position",
            examples: &["pause"],
            category: CommandCategory::Transport,
        },
        Arc::new(cmd_pause),
    );

    registry.register(
        CommandInfo {
            name: "record",
            aliases: &["rec", "r"],
            usage: "record",
            description: "Start recording on armed tracks",
            examples: &["record"],
            category: CommandCategory::Transport,
        },
        Arc::new(cmd_record),
    );

    registry.register(
        CommandInfo {
            name: "seek",
            aliases: &["goto"],
            usage: "seek <position>",
            description: "Seek to position (beats or bars:beats)",
            examples: &["seek 0", "seek 16", "seek 4:1"],
            category: CommandCategory::Transport,
        },
        Arc::new(cmd_seek),
    );

    registry.register(
        CommandInfo {
            name: "tempo",
            aliases: &["bpm"],
            usage: "tempo <bpm>",
            description: "Set tempo in BPM",
            examples: &["tempo 120", "tempo 140.5"],
            category: CommandCategory::Transport,
        },
        Arc::new(cmd_tempo),
    );

    registry.register(
        CommandInfo {
            name: "loop",
            aliases: &[],
            usage: "loop [start] [end] | loop on | loop off",
            description: "Set loop region or toggle looping",
            examples: &["loop 0 16", "loop on", "loop off"],
            category: CommandCategory::Transport,
        },
        Arc::new(cmd_loop),
    );

    registry.register(
        CommandInfo {
            name: "position",
            aliases: &["pos", "where"],
            usage: "position",
            description: "Show current playback position",
            examples: &["position"],
            category: CommandCategory::Transport,
        },
        Arc::new(cmd_position),
    );
}

fn cmd_play(ctx: &mut CommandContext, _cmd: &ParsedCommand) -> CommandResult {
    ctx.host.send_command(Command::Play)
        .map_err(|e| format!("Failed to send command: {:?}", e))?;
    Ok(CommandOutput::text("▶ Playing"))
}

fn cmd_stop(ctx: &mut CommandContext, _cmd: &ParsedCommand) -> CommandResult {
    ctx.host.send_command(Command::Stop)
        .map_err(|e| format!("Failed: {:?}", e))?;
    Ok(CommandOutput::text("⏹ Stopped"))
}

fn cmd_pause(_ctx: &mut CommandContext, _cmd: &ParsedCommand) -> CommandResult {
    // TODO: Implement pause command when available in core
    Ok(CommandOutput::text("⏸ Paused (not yet implemented)"))
}

fn cmd_record(_ctx: &mut CommandContext, _cmd: &ParsedCommand) -> CommandResult {
    // TODO: Implement record command when available in core
    Ok(CommandOutput::text("⏺ Recording (not yet implemented)"))
}

fn cmd_seek(_ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    let _pos_str = cmd.require_arg(0, "position")?;
    
    // TODO: Implement seek command when available in core
    Ok(CommandOutput::text("Seek (not yet implemented)"))
}

fn cmd_tempo(ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    let bpm: f64 = cmd.parse_arg(0, "bpm")?;
    
    if bpm < 20.0 || bpm > 999.0 {
        return Err("Tempo must be between 20 and 999 BPM".into());
    }
    
    ctx.host.send_command(Command::SetTempo(bpm))
        .map_err(|e| format!("Failed: {:?}", e))?;
    ctx.project.tempo_map.set_default_tempo(bpm);
    
    Ok(CommandOutput::text(format!("Tempo: {:.1} BPM", bpm)))
}

fn cmd_loop(_ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    match cmd.arg(0) {
        Some("on") => {
            // TODO: Implement loop enable when available in core
            Ok(CommandOutput::text("Loop: ON (not yet implemented)"))
        }
        Some("off") => {
            // TODO: Implement loop disable when available in core
            Ok(CommandOutput::text("Loop: OFF (not yet implemented)"))
        }
        Some(start_str) => {
            let start: f64 = start_str.parse().map_err(|_| "Invalid start position")?;
            let end: f64 = cmd.parse_arg(1, "end")?;
            
            if end <= start {
                return Err("Loop end must be after start".into());
            }
            
            // TODO: Implement loop region when available in core
            Ok(CommandOutput::text(format!("Loop: {:.1} - {:.1} beats (not yet implemented)", start, end)))
        }
        None => Err("Usage: loop <start> <end> | loop on | loop off".into()),
    }
}

fn cmd_position(_ctx: &mut CommandContext, _cmd: &ParsedCommand) -> CommandResult {
    // TODO: Query actual position from host when available
    Ok(CommandOutput::text("Position: 1:1.000 (0.0 beats)"))
}
