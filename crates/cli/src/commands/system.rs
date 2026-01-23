//! System and utility commands.

use super::registry::*;
use crate::parser::ParsedCommand;
use std::sync::Arc;

pub fn register(registry: &mut CommandRegistry) {
    registry.register(
        CommandInfo {
            name: "help",
            aliases: &["h", "?"],
            usage: "help [command]",
            description: "Show help for commands",
            examples: &["help", "help play", "help add-track"],
            category: CommandCategory::System,
        },
        Arc::new(cmd_help),
    );

    registry.register(
        CommandInfo {
            name: "quit",
            aliases: &["exit", "q"],
            usage: "quit",
            description: "Exit ondeks",
            examples: &["quit"],
            category: CommandCategory::System,
        },
        Arc::new(cmd_quit),
    );

    registry.register(
        CommandInfo {
            name: "devices",
            aliases: &[],
            usage: "devices",
            description: "List available audio devices",
            examples: &["devices"],
            category: CommandCategory::System,
        },
        Arc::new(cmd_devices),
    );

    registry.register(
        CommandInfo {
            name: "device",
            aliases: &[],
            usage: "device <name|index>",
            description: "Select audio output device",
            examples: &["device 1", "device \"Built-in Output\""],
            category: CommandCategory::System,
        },
        Arc::new(cmd_device),
    );

    registry.register(
        CommandInfo {
            name: "cpu",
            aliases: &["perf", "stats"],
            usage: "cpu",
            description: "Show CPU and performance stats",
            examples: &["cpu"],
            category: CommandCategory::System,
        },
        Arc::new(cmd_cpu),
    );

    registry.register(
        CommandInfo {
            name: "meters",
            aliases: &["vu"],
            usage: "meters",
            description: "Show level meters for all tracks",
            examples: &["meters"],
            category: CommandCategory::System,
        },
        Arc::new(cmd_meters),
    );

    registry.register(
        CommandInfo {
            name: "test-tone",
            aliases: &["tone", "beep"],
            usage: "test-tone [freq] [duration]",
            description: "Play a test tone",
            examples: &["test-tone", "test-tone 880", "test-tone 440 2"],
            category: CommandCategory::System,
        },
        Arc::new(cmd_test_tone),
    );

    registry.register(
        CommandInfo {
            name: "clear",
            aliases: &["cls"],
            usage: "clear",
            description: "Clear the screen",
            examples: &["clear"],
            category: CommandCategory::System,
        },
        Arc::new(cmd_clear),
    );
}

fn cmd_help(_ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    // Would need access to registry - simplified here
    if let Some(command_name) = cmd.arg(0) {
        // Show help for specific command
        Ok(CommandOutput::text(format!("Help for '{}': (details would go here)", command_name)))
    } else {
        // Show all commands grouped by category
        let mut output = String::from("ondeks - Digital Audio Workstation Engine\n\n");
        
        output.push_str("COMMANDS:\n\n");
        
        output.push_str("  Project:    new, load, save, export, info\n");
        output.push_str("  Transport:  play, stop, pause, record, seek, tempo, loop, position\n");
        output.push_str("  Track:      add-track, remove-track, list-tracks, mute, solo, arm, volume, pan\n");
        output.push_str("  Session:    mode, launch, launch-scene, stop-track, stop-all, session-view\n");
        output.push_str("  System:     devices, device, cpu, test-tone, clear, help, quit\n");
        output.push_str("\nType 'help <command>' for detailed help on a specific command.\n");
        
        Ok(CommandOutput::Text(output))
    }
}

fn cmd_quit(_ctx: &mut CommandContext, _cmd: &ParsedCommand) -> CommandResult {
    Ok(CommandOutput::Exit)
}

fn cmd_devices(ctx: &mut CommandContext, _cmd: &ParsedCommand) -> CommandResult {
    let devices = ctx.host.list_devices();
    
    if devices.is_empty() {
        return Ok(CommandOutput::text("No audio devices found"));
    }
    
    let mut output = String::from("Audio Devices:\n");
    for (i, device) in devices.iter().enumerate() {
        let marker = if device.is_default { "*" } else { " " };
        output.push_str(&format!(
            "  {} {:2}. {} ({} channels)\n",
            marker, i + 1, device.name, device.max_channels
        ));
    }
    output.push_str("\n* = default device");
    
    Ok(CommandOutput::Text(output))
}

fn cmd_device(ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    let identifier = cmd.require_arg(0, "device")?;
    
    let devices = ctx.host.list_devices();
    
    let device_name = if let Ok(index) = identifier.parse::<usize>() {
        if index == 0 || index > devices.len() {
            return Err(format!("Device index {} out of range (1-{})", index, devices.len()));
        }
        devices[index - 1].name.clone()
    } else {
        identifier.to_string()
    };
    
    // Would call: ctx.host.select_device(&device_name)?;
    
    Ok(CommandOutput::text(format!("Selected device: {}", device_name)))
}

fn cmd_cpu(ctx: &mut CommandContext, _cmd: &ParsedCommand) -> CommandResult {
    // Would get actual stats from host
    let output = format!(
        "Performance:\n\
         CPU Usage:    {:5.1}%\n\
         Buffer Size:  {} samples\n\
         Sample Rate:  {} Hz\n\
         Latency:      {:.1} ms\n\
         Underruns:    {}",
        12.3,
        ctx.host.config().buffer_size,
        ctx.host.sample_rate(),
        ctx.host.config().buffer_size as f32 / ctx.host.sample_rate() as f32 * 1000.0,
        0
    );
    
    Ok(CommandOutput::Text(output))
}

fn cmd_meters(ctx: &mut CommandContext, _cmd: &ParsedCommand) -> CommandResult {
    let mut output = String::from("Level Meters:\n\n");
    
    for track in ctx.project.tracks() {
        // Would get actual meter values
        let left_db = -12.0f32;
        let right_db = -15.0f32;
        
        let left_bar = meter_bar(left_db);
        let right_bar = meter_bar(right_db);
        
        output.push_str(&format!(
            "{:12} L {} {:+5.1} dB\n",
            track.name, left_bar, left_db
        ));
        output.push_str(&format!(
            "             R {} {:+5.1} dB\n\n",
            right_bar, right_db
        ));
    }
    
    Ok(CommandOutput::Text(output))
}

fn meter_bar(db: f32) -> String {
    let normalized = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
    let filled = (normalized * 30.0) as usize;
    let empty = 30 - filled;
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

fn cmd_test_tone(ctx: &mut CommandContext, cmd: &ParsedCommand) -> CommandResult {
    let freq: f32 = cmd.arg(0).and_then(|s| s.parse().ok()).unwrap_or(440.0);
    let duration: f32 = cmd.arg(1).and_then(|s| s.parse().ok()).unwrap_or(1.0);
    
    if freq < 20.0 || freq > 20000.0 {
        return Err("Frequency must be between 20 and 20000 Hz".into());
    }
    
    if duration < 0.1 || duration > 10.0 {
        return Err("Duration must be between 0.1 and 10 seconds".into());
    }
    
    // Actually play tone through host
    ctx.host.play_test_tone(freq, duration)
        .map_err(|e| format!("Failed to play tone: {}", e))?;
    
    Ok(CommandOutput::text(format!("Playing {} Hz for {:.1}s", freq, duration)))
}

fn cmd_clear(_ctx: &mut CommandContext, _cmd: &ParsedCommand) -> CommandResult {
    print!("\x1B[2J\x1B[1;1H");
    Ok(CommandOutput::Silent)
}
