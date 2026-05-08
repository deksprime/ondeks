//! Session view commands.

use super::registry::*;
use crate::parser::ParsedCommand;
use ondeks_core::session::{ClipPlayback, SlotState, ViewMode};
use ondeks_core::transport::Beats;
use ondeks_core::NodeId;
use std::sync::Arc;

/// Stub playback used for slots that have no playable clip (or no instrument).
/// The launcher needs *some* playback to track state; the empty notes list
/// produces no MIDI events when the engine advances it.
fn stub_playback(length_beats: f64) -> ClipPlayback {
    ClipPlayback {
        target_node: NodeId::from_raw(0),
        length_beats,
        notes: Vec::new(),
    }
}

pub fn register(registry: &mut CommandRegistry) {
    registry.register(
        CommandInfo {
            name: "mode",
            aliases: &[],
            usage: "mode <session|arrangement>",
            description: "Switch between session and arrangement view",
            examples: &["mode session", "mode arrangement"],
            category: CommandCategory::Session,
        },
        Arc::new(cmd_mode),
    );

    registry.register(
        CommandInfo {
            name: "launch",
            aliases: &["l"],
            usage: "launch <track> <scene>",
            description: "Launch a clip in session view",
            examples: &["launch 1 1", "launch Bass 3"],
            category: CommandCategory::Session,
        },
        Arc::new(cmd_launch),
    );

    registry.register(
        CommandInfo {
            name: "launch-scene",
            aliases: &["ls", "scene"],
            usage: "launch-scene <scene>",
            description: "Launch all clips in a scene",
            examples: &["launch-scene 1", "scene 2"],
            category: CommandCategory::Session,
        },
        Arc::new(cmd_launch_scene),
    );

    registry.register(
        CommandInfo {
            name: "stop-track",
            aliases: &["st"],
            usage: "stop-track <track>",
            description: "Stop clips on a track",
            examples: &["stop-track 1", "stop-track Bass"],
            category: CommandCategory::Session,
        },
        Arc::new(cmd_stop_track),
    );

    registry.register(
        CommandInfo {
            name: "stop-all",
            aliases: &["panic"],
            usage: "stop-all",
            description: "Stop all clips",
            examples: &["stop-all"],
            category: CommandCategory::Session,
        },
        Arc::new(cmd_stop_all),
    );

    registry.register(
        CommandInfo {
            name: "session-view",
            aliases: &["sv"],
            usage: "session-view",
            description: "Show session view grid",
            examples: &["session-view"],
            category: CommandCategory::Session,
        },
        Arc::new(cmd_session_view),
    );
}

fn cmd_mode(ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    let mode_str = cmd.require_arg(0, "mode")?;
    
    match mode_str.to_lowercase().as_str() {
        "session" | "s" => {
            ctx.view_manager.switch_to_session();
            Ok(CommandOutput::text("Switched to Session view"))
        }
        "arrangement" | "arrange" | "a" => {
            ctx.view_manager.switch_to_arrangement();
            Ok(CommandOutput::text("Switched to Arrangement view"))
        }
        _ => Err("Invalid mode. Use 'session' or 'arrangement'".into()),
    }
}

fn cmd_launch(ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    if ctx.view_manager.mode != ViewMode::Session {
        return Err("Must be in session mode to launch clips. Use: mode session".into());
    }
    
    let track: usize = cmd.parse_arg(0, "track")?;
    let scene: usize = cmd.parse_arg(1, "scene")?;
    
    if track == 0 || scene == 0 {
        return Err("Track and scene indices start at 1".into());
    }
    
    let ts_numerator = ctx.project.time_signature.numerator;
    let playback = ctx
        .project
        .clip_playback_for_slot(track - 1, scene - 1)
        .unwrap_or_else(|| stub_playback(4.0));

    ctx.view_manager.session.launch_clip(
        track - 1,
        scene - 1,
        playback,
        Beats(0.0), // Would need actual position
        ts_numerator,
    );

    Ok(CommandOutput::text(format!("Launched clip at track {}, scene {}", track, scene)))
}

fn cmd_launch_scene(ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    if ctx.view_manager.mode != ViewMode::Session {
        return Err("Must be in session mode".into());
    }
    
    let scene: usize = cmd.parse_arg(0, "scene")?;
    if scene == 0 {
        return Err("Scene index starts at 1".into());
    }
    
    let ts_numerator = ctx.project.time_signature.numerator;
    let scene_idx = scene - 1;

    for (track_idx, playback) in ctx.project.scene_playbacks(scene_idx) {
        ctx.view_manager.session.launch_clip(
            track_idx,
            scene_idx,
            playback,
            Beats(0.0),
            ts_numerator,
        );
    }

    Ok(CommandOutput::text(format!("Launched scene {}", scene)))
}

fn cmd_stop_track(ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    let track: usize = cmd.parse_arg(0, "track")?;
    if track == 0 {
        return Err("Track index starts at 1".into());
    }
    
    ctx.view_manager.session.stop_track(track - 1);
    Ok(CommandOutput::text(format!("Stopped track {}", track)))
}

fn cmd_stop_all(ctx: &mut CommandContext, _cmd: &ParsedCommand) -> CommandResult {
    ctx.view_manager.session.stop_all();
    Ok(CommandOutput::text("Stopped all clips"))
}

fn cmd_session_view(ctx: &mut CommandContext, _cmd: &ParsedCommand) -> CommandResult {
    let mut output = String::from("Session View:\n\n");
    
    // Header
    output.push_str("     ");
    for track in ctx.project.tracks().iter() {
        let name: String = track.name.chars().take(6).collect();
        output.push_str(&format!(" {:^6} ", name));
    }
    output.push('\n');
    
    // Grid
    for scene in 0..ctx.project.scene_count() {
        output.push_str(&format!("{:3}. ", scene + 1));
        
        for (track_idx, _track) in ctx.project.tracks().iter().enumerate() {
            let state = ctx.view_manager.session.slot_state(track_idx, scene);
            let symbol = match state {
                SlotState::Empty => "  ·   ",
                SlotState::Stopped => "  ■   ",
                SlotState::Queued => "  ◆   ",
                SlotState::Playing => "  ▶   ",
                SlotState::Recording => "  ●   ",
            };
            output.push_str(symbol);
        }
        output.push('\n');
    }
    
    Ok(CommandOutput::Text(output))
}
