//! Track management commands.

use super::registry::*;
use crate::parser::ParsedCommand;
use ondeks_core::project::{Track, TrackType};
use std::sync::Arc;

pub fn register(registry: &mut CommandRegistry) {
    registry.register(
        CommandInfo {
            name: "add-track",
            aliases: &["track", "at"],
            usage: "add-track <type> <name>",
            description: "Add a new track (types: midi, audio, group, return)",
            examples: &["add-track midi \"Bass\"", "add-track audio Vocals", "add-track return Reverb"],
            category: CommandCategory::Track,
        },
        Arc::new(cmd_add_track),
    );

    registry.register(
        CommandInfo {
            name: "remove-track",
            aliases: &["rm-track", "delete-track"],
            usage: "remove-track <id|name>",
            description: "Remove a track",
            examples: &["remove-track 1", "remove-track Bass"],
            category: CommandCategory::Track,
        },
        Arc::new(cmd_remove_track),
    );

    registry.register(
        CommandInfo {
            name: "list-tracks",
            aliases: &["tracks", "lt"],
            usage: "list-tracks",
            description: "List all tracks",
            examples: &["list-tracks"],
            category: CommandCategory::Track,
        },
        Arc::new(cmd_list_tracks),
    );

    registry.register(
        CommandInfo {
            name: "mute",
            aliases: &["m"],
            usage: "mute <track>",
            description: "Toggle mute on a track",
            examples: &["mute 1", "mute Bass"],
            category: CommandCategory::Track,
        },
        Arc::new(cmd_mute),
    );

    registry.register(
        CommandInfo {
            name: "solo",
            aliases: &["so"],
            usage: "solo <track>",
            description: "Toggle solo on a track",
            examples: &["solo 1", "solo Drums"],
            category: CommandCategory::Track,
        },
        Arc::new(cmd_solo),
    );

    registry.register(
        CommandInfo {
            name: "arm",
            aliases: &[],
            usage: "arm <track>",
            description: "Toggle record arm on a track",
            examples: &["arm 1", "arm Vocals"],
            category: CommandCategory::Track,
        },
        Arc::new(cmd_arm),
    );

    registry.register(
        CommandInfo {
            name: "volume",
            aliases: &["vol", "v"],
            usage: "volume <track> <dB>",
            description: "Set track volume in dB",
            examples: &["volume 1 -6", "volume Bass 0", "volume 2 -inf"],
            category: CommandCategory::Track,
        },
        Arc::new(cmd_volume),
    );

    registry.register(
        CommandInfo {
            name: "pan",
            aliases: &[],
            usage: "pan <track> <value>",
            description: "Set track pan (-1.0 = left, 0 = center, 1.0 = right)",
            examples: &["pan 1 -0.5", "pan Drums 0", "pan 2 1.0"],
            category: CommandCategory::Track,
        },
        Arc::new(cmd_pan),
    );

    registry.register(
        CommandInfo {
            name: "rename-track",
            aliases: &[],
            usage: "rename-track <track> <new-name>",
            description: "Rename a track",
            examples: &["rename-track 1 \"Lead Synth\""],
            category: CommandCategory::Track,
        },
        Arc::new(cmd_rename),
    );
}

fn find_track<'a>(project: &'a mut ondeks_core::project::Project, identifier: &str) -> Result<&'a mut Track, String> {
    // Try as index first
    if let Ok(index) = identifier.parse::<usize>() {
        let tracks = project.tracks();
        if index == 0 || index > tracks.len() {
            return Err(format!("Track index {} out of range (1-{})", index, tracks.len()));
        }
        let id = tracks[index - 1].id;
        return project.get_track_mut(id).ok_or_else(|| "Track not found".into());
    }
    
    // Try as name
    let id = project.tracks()
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(identifier))
        .map(|t| t.id)
        .ok_or_else(|| format!("Track '{}' not found", identifier))?;
    
    project.get_track_mut(id).ok_or_else(|| "Track not found".into())
}

fn cmd_add_track(ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    let type_str = cmd.require_arg(0, "type")?;
    let name = cmd.arg(1).unwrap_or("New Track");
    
    let track_type = match type_str.to_lowercase().as_str() {
        "midi" => TrackType::Midi,
        "audio" => TrackType::Audio,
        "group" => TrackType::Group,
        "return" | "aux" | "bus" => TrackType::Return,
        _ => return Err(format!("Unknown track type: '{}'. Use: midi, audio, group, return", type_str)),
    };
    
    let id = ctx.project.add_track(track_type, name);
    let index = ctx.project.tracks().iter().position(|t| t.id == id).unwrap() + 1;
    
    Ok(CommandOutput::text(format!("Added {:?} track '{}' (#{}) ", track_type, name, index)))
}

fn cmd_remove_track(ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    let identifier = cmd.require_arg(0, "track")?;
    let track = find_track(ctx.project, identifier)?;
    let name = track.name.clone();
    let id = track.id;
    
    ctx.project.remove_track(id)
        .map_err(|e| format!("{}", e))?;
    
    Ok(CommandOutput::text(format!("Removed track '{}'", name)))
}

fn cmd_list_tracks(ctx: &mut CommandContext, _cmd: &ParsedCommand) -> CommandResult {
    let mut output = String::from("Tracks:\n");
    
    for (i, track) in ctx.project.tracks().iter().enumerate() {
        let mute = if track.muted { "M" } else { " " };
        let solo = if track.soloed { "S" } else { " " };
        let arm = if track.armed { "R" } else { " " };
        let type_char = match track.track_type {
            TrackType::Midi => "♪",
            TrackType::Audio => "♫",
            TrackType::Group => "▤",
            TrackType::Return => "↩",
            TrackType::Master => "★",
        };
        
        output.push_str(&format!(
            "  {:2}. {} [{}{}{}] {:>6.1} dB  {:+.1}  {}\n",
            i + 1,
            type_char,
            mute, solo, arm,
            track.volume_db,
            track.pan,
            track.name,
        ));
    }
    
    Ok(CommandOutput::Text(output))
}

fn cmd_mute(ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    let identifier = cmd.require_arg(0, "track")?;
    let track = find_track(ctx.project, identifier)?;
    track.muted = !track.muted;
    let state = if track.muted { "muted" } else { "unmuted" };
    Ok(CommandOutput::text(format!("Track '{}' {}", track.name, state)))
}

fn cmd_solo(ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    let identifier = cmd.require_arg(0, "track")?;
    let track = find_track(ctx.project, identifier)?;
    track.soloed = !track.soloed;
    let state = if track.soloed { "soloed" } else { "unsoloed" };
    Ok(CommandOutput::text(format!("Track '{}' {}", track.name, state)))
}

fn cmd_arm(ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    let identifier = cmd.require_arg(0, "track")?;
    let track = find_track(ctx.project, identifier)?;
    track.armed = !track.armed;
    let state = if track.armed { "armed" } else { "disarmed" };
    Ok(CommandOutput::text(format!("Track '{}' {}", track.name, state)))
}

fn cmd_volume(ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    let identifier = cmd.require_arg(0, "track")?;
    let db_str = cmd.require_arg(1, "dB")?;
    
    let db: f32 = if db_str == "-inf" {
        f32::NEG_INFINITY
    } else {
        db_str.parse().map_err(|_| "Invalid dB value")?
    };
    
    let track = find_track(ctx.project, identifier)?;
    track.volume_db = db;
    
    Ok(CommandOutput::text(format!("Track '{}' volume: {:.1} dB", track.name, db)))
}

fn cmd_pan(ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    let identifier = cmd.require_arg(0, "track")?;
    let pan: f32 = cmd.parse_arg(1, "pan")?;
    
    if pan < -1.0 || pan > 1.0 {
        return Err("Pan must be between -1.0 (left) and 1.0 (right)".into());
    }
    
    let track = find_track(ctx.project, identifier)?;
    track.pan = pan;
    
    let pos = if pan < -0.01 {
        format!("{:.0}% L", pan.abs() * 100.0)
    } else if pan > 0.01 {
        format!("{:.0}% R", pan * 100.0)
    } else {
        "C".into()
    };
    
    Ok(CommandOutput::text(format!("Track '{}' pan: {}", track.name, pos)))
}

fn cmd_rename(ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    let identifier = cmd.require_arg(0, "track")?;
    let new_name = cmd.require_arg(1, "new-name")?;
    
    let track = find_track(ctx.project, identifier)?;
    let old_name = track.name.clone();
    track.name = new_name.to_string();
    
    Ok(CommandOutput::text(format!("Renamed '{}' to '{}'", old_name, new_name)))
}
