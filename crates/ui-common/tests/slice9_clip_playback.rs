//! Slice 9: ui-common — verify the session view model surfaces engine slot
//! states and the session command set carries the new launch verbs.
//!
//! End-to-end engine integration lives in
//! `crates/core/tests/slice9_clip_playback.rs`. These tests cover the UI
//! seam: a slot mirror map → `SessionViewModel::from_project` → per-slot
//! `state` field → `SlotViewModel::state_icon` reflects the engine.

use ondeks_core::project::{Clip, MidiClip, Project, TrackType};
use ondeks_core::session::SlotState;
use ondeks_core::transport::Beats;
use ondeks_ui_common::{
    SessionViewModel,
    commands::SessionCommand,
};

fn project_with_one_clip() -> Project {
    let mut p = Project::new("test");
    let track_id = p.add_track(TrackType::Midi, "Lead");
    let clip = MidiClip::new("Pattern", Beats(4.0));
    let clip_id = clip.id();
    p.add_clip(Clip::Midi(clip));
    p.get_track_mut(track_id)
        .unwrap()
        .set_session_slot(0, Some(clip_id));
    p
}

#[test]
fn session_view_model_reflects_playing_slot() {
    let project = project_with_one_clip();

    // Track 0 / scene 0 — engine reports Playing via the closure.
    let vm = SessionViewModel::from_project(
        &project,
        |t, s| {
            if t == 0 && s == 0 {
                SlotState::Playing
            } else {
                SlotState::Empty
            }
        },
        |_, _| false,
    );
    let slot = vm.get_slot(0, 0).expect("slot exists");
    assert_eq!(slot.state, SlotState::Playing);
    assert_eq!(slot.state_icon(), "▶");
}

#[test]
fn session_view_model_shows_queued_until_engine_advances() {
    let project = project_with_one_clip();
    let vm = SessionViewModel::from_project(
        &project,
        |t, s| {
            if t == 0 && s == 0 {
                SlotState::Queued
            } else {
                SlotState::Empty
            }
        },
        |_, _| false,
    );
    assert_eq!(vm.get_slot(0, 0).unwrap().state, SlotState::Queued);
    assert_eq!(vm.get_slot(0, 0).unwrap().state_icon(), "◆");
}

#[test]
fn empty_slot_state_is_empty_regardless_of_closure() {
    // Even if the closure claims the slot is Playing, an empty session_slot
    // (no clip attached) renders as Empty — `from_project` defers to the
    // project's slot contents first.
    let mut project = Project::new("test");
    let _track_id = project.add_track(TrackType::Midi, "Lead");
    // No clip attached → slot is empty.

    let vm = SessionViewModel::from_project(
        &project,
        |_, _| SlotState::Playing,
        |_, _| false,
    );
    let slot = vm.get_slot(0, 0).expect("slot row exists for scene 0");
    assert_eq!(slot.state, SlotState::Empty);
}

#[test]
fn session_command_describes_each_launch_verb() {
    assert_eq!(
        SessionCommand::LaunchClip { track: 0, scene: 0 }.description(),
        "Launch Clip"
    );
    assert_eq!(
        SessionCommand::StopTrack { track: 0 }.description(),
        "Stop Track"
    );
    assert_eq!(
        SessionCommand::LaunchScene { scene: 0 }.description(),
        "Launch Scene"
    );
    assert_eq!(SessionCommand::StopAll.description(), "Stop All Clips");
}
