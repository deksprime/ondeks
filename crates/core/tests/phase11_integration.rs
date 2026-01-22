//! Integration tests for Phase 11: Session & Arrangement
//!
//! These tests verify that clip launcher, arrangement playback, and view management work correctly.

use ondeks_core::session::{ClipLauncher, SlotState, LaunchQuantize, LaunchEvent};
use ondeks_core::arrangement::{ArrangementPlayback, ArrangementEvent};
use ondeks_core::session::{ViewManager, ViewMode};
use ondeks_core::project::{Project, TrackType, Clip, MidiClip};
use ondeks_core::transport::Beats;

#[test]
fn clip_launcher_quantize_none() {
    let quantize = LaunchQuantize::None;
    let current = Beats(1.3);
    let next = quantize.next_position(current, 4);
    assert_eq!(next.0, 1.3);
}

#[test]
fn clip_launcher_quantize_bar() {
    let quantize = LaunchQuantize::Bar;
    let current = Beats(1.3);
    let next = quantize.next_position(current, 4);
    assert_eq!(next.0, 4.0); // Next bar boundary
    
    let current2 = Beats(4.0);
    let next2 = quantize.next_position(current2, 4);
    assert_eq!(next2.0, 4.0); // Already on bar boundary
    
    let current3 = Beats(5.7);
    let next3 = quantize.next_position(current3, 4);
    assert_eq!(next3.0, 8.0); // Next bar boundary
}

#[test]
fn clip_launcher_quantize_beat() {
    let quantize = LaunchQuantize::Beat;
    let current = Beats(1.3);
    let next = quantize.next_position(current, 4);
    assert_eq!(next.0, 2.0); // Next beat boundary
    
    let current2 = Beats(2.0);
    let next2 = quantize.next_position(current2, 4);
    assert_eq!(next2.0, 2.0); // Already on beat boundary
}

#[test]
fn clip_launcher_quantize_half_beat() {
    let quantize = LaunchQuantize::HalfBeat;
    let current = Beats(1.3);
    let next = quantize.next_position(current, 4);
    assert_eq!(next.0, 1.5); // Next half-beat boundary
    
    let current2 = Beats(1.5);
    let next2 = quantize.next_position(current2, 4);
    assert_eq!(next2.0, 1.5); // Already on half-beat boundary
}

#[test]
fn clip_launcher_slot_state() {
    let mut launcher = ClipLauncher::new();
    
    assert_eq!(launcher.slot_state(0, 0), SlotState::Empty);
    
    launcher.set_slot_state(0, 0, SlotState::Stopped);
    assert_eq!(launcher.slot_state(0, 0), SlotState::Stopped);
    
    launcher.set_slot_state(0, 0, SlotState::Playing);
    assert_eq!(launcher.slot_state(0, 0), SlotState::Playing);
}

#[test]
fn clip_launcher_launch_clip() {
    let mut launcher = ClipLauncher::new();
    launcher.set_slot_state(0, 0, SlotState::Stopped);
    
    let current_pos = Beats(1.0);
    launcher.launch_clip(0, 0, current_pos, 4);
    
    // Should be queued
    assert_eq!(launcher.slot_state(0, 0), SlotState::Queued);
    
    // Process at trigger time
    let events = launcher.process(Beats(4.0));
    assert_eq!(events.len(), 1);
    match &events[0] {
        LaunchEvent::ClipStarted { track, scene } => {
            assert_eq!(*track, 0);
            assert_eq!(*scene, 0);
        }
        _ => panic!("Expected ClipStarted event"),
    }
    
    // Should now be playing
    assert_eq!(launcher.slot_state(0, 0), SlotState::Playing);
}

#[test]
fn clip_launcher_stop_playing_clip_on_track() {
    let mut launcher = ClipLauncher::new();
    launcher.set_slot_state(0, 0, SlotState::Playing);
    launcher.set_slot_state(0, 1, SlotState::Playing);
    launcher.set_slot_state(1, 0, SlotState::Playing); // Different track
    
    // Launch new clip on track 0
    launcher.launch_clip(0, 2, Beats(1.0), 4);
    
    // Track 0, scene 0 should be stopped
    assert_eq!(launcher.slot_state(0, 0), SlotState::Stopped);
    // Track 0, scene 1 should be stopped
    assert_eq!(launcher.slot_state(0, 1), SlotState::Stopped);
    // Track 1, scene 0 should still be playing
    assert_eq!(launcher.slot_state(1, 0), SlotState::Playing);
}

#[test]
fn clip_launcher_launch_scene() {
    let mut launcher = ClipLauncher::new();
    launcher.set_slot_state(0, 0, SlotState::Stopped);
    launcher.set_slot_state(1, 0, SlotState::Stopped);
    launcher.set_slot_state(2, 0, SlotState::Empty); // Empty slot
    
    launcher.launch_scene(0, 3, Beats(1.0), 4);
    
    // Should queue clips in scene 0 for tracks 0 and 1
    assert_eq!(launcher.slot_state(0, 0), SlotState::Queued);
    assert_eq!(launcher.slot_state(1, 0), SlotState::Queued);
    assert_eq!(launcher.slot_state(2, 0), SlotState::Empty); // Empty slot unchanged
}

#[test]
fn clip_launcher_stop_track() {
    let mut launcher = ClipLauncher::new();
    launcher.set_slot_state(0, 0, SlotState::Playing);
    launcher.set_slot_state(0, 1, SlotState::Queued);
    launcher.set_slot_state(1, 0, SlotState::Playing); // Different track
    
    launcher.stop_track(0);
    
    // Track 0 clips should be stopped
    assert_eq!(launcher.slot_state(0, 0), SlotState::Stopped);
    assert_eq!(launcher.slot_state(0, 1), SlotState::Stopped);
    // Track 1 should be unchanged
    assert_eq!(launcher.slot_state(1, 0), SlotState::Playing);
}

#[test]
fn clip_launcher_stop_all() {
    let mut launcher = ClipLauncher::new();
    launcher.set_slot_state(0, 0, SlotState::Playing);
    launcher.set_slot_state(0, 1, SlotState::Queued);
    launcher.set_slot_state(1, 0, SlotState::Playing);
    
    launcher.stop_all();
    
    // All playing/queued clips should be stopped
    assert_eq!(launcher.slot_state(0, 0), SlotState::Stopped);
    assert_eq!(launcher.slot_state(0, 1), SlotState::Stopped);
    assert_eq!(launcher.slot_state(1, 0), SlotState::Stopped);
    
    // Queued starts should be cleared
    assert!(launcher.process(Beats(100.0)).is_empty());
}

#[test]
fn arrangement_playback_clip_starts() {
    let mut project = Project::new("Test");
    let track_id = project.add_track(TrackType::Midi, "Track 1");
    
    let clip = MidiClip::new("Clip 1", Beats(4.0));
    let clip_id = project.add_clip(Clip::Midi(clip));
    project.place_clip(track_id, clip_id, Beats(0.0)).unwrap();
    
    let mut playback = ArrangementPlayback::new();
    
    // At position 0.0, clip should start
    let events = playback.update(&project, Beats(0.0));
    assert_eq!(events.len(), 1);
    match &events[0] {
        ArrangementEvent::ClipStarted { track, clip } => {
            assert_eq!(*track, track_id);
            assert_eq!(*clip, clip_id);
        }
        _ => panic!("Expected ClipStarted event"),
    }
    
    // Should have active clip
    let active = playback.active_clips(track_id);
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].clip_id, clip_id);
    assert_eq!(active[0].start_position, Beats(0.0));
    assert_eq!(active[0].end_position, Beats(4.0));
}

#[test]
fn arrangement_playback_clip_stops() {
    let mut project = Project::new("Test");
    let track_id = project.add_track(TrackType::Midi, "Track 1");
    
    let clip = MidiClip::new("Clip 1", Beats(4.0));
    let clip_id = project.add_clip(Clip::Midi(clip));
    project.place_clip(track_id, clip_id, Beats(0.0)).unwrap();
    
    let mut playback = ArrangementPlayback::new();
    
    // Start clip
    playback.update(&project, Beats(0.0));
    
    // At position 4.0, clip should end
    let events = playback.update(&project, Beats(4.0));
    assert_eq!(events.len(), 1);
    match &events[0] {
        ArrangementEvent::ClipEnded { track, clip } => {
            assert_eq!(*track, track_id);
            assert_eq!(*clip, clip_id);
        }
        _ => panic!("Expected ClipEnded event"),
    }
    
    // Should have no active clips
    let active = playback.active_clips(track_id);
    assert!(active.is_empty());
}

#[test]
fn arrangement_playback_multiple_clips() {
    let mut project = Project::new("Test");
    let track_id = project.add_track(TrackType::Midi, "Track 1");
    
    let clip1 = MidiClip::new("Clip 1", Beats(4.0));
    let clip1_id = project.add_clip(Clip::Midi(clip1));
    project.place_clip(track_id, clip1_id, Beats(0.0)).unwrap();
    
    let clip2 = MidiClip::new("Clip 2", Beats(4.0));
    let clip2_id = project.add_clip(Clip::Midi(clip2));
    project.place_clip(track_id, clip2_id, Beats(8.0)).unwrap();
    
    let mut playback = ArrangementPlayback::new();
    
    // At position 0.0, clip1 should start
    playback.update(&project, Beats(0.0));
    assert_eq!(playback.active_clips(track_id).len(), 1);
    
    // At position 4.0, clip1 should end
    playback.update(&project, Beats(4.0));
    assert_eq!(playback.active_clips(track_id).len(), 0);
    
    // At position 8.0, clip2 should start
    playback.update(&project, Beats(8.0));
    assert_eq!(playback.active_clips(track_id).len(), 1);
    assert_eq!(playback.active_clips(track_id)[0].clip_id, clip2_id);
}

#[test]
fn arrangement_playback_muted_clip() {
    let mut project = Project::new("Test");
    let track_id = project.add_track(TrackType::Midi, "Track 1");
    
    let clip = MidiClip::new("Clip 1", Beats(4.0));
    let clip_id = project.add_clip(Clip::Midi(clip));
    project.place_clip(track_id, clip_id, Beats(0.0)).unwrap();
    
    // Mute the clip
    let track = project.get_track_mut(track_id).unwrap();
    track.arrangement_clips[0].muted = true;
    
    let mut playback = ArrangementPlayback::new();
    
    // At position 0.0, muted clip should not start
    let events = playback.update(&project, Beats(0.0));
    assert!(events.is_empty());
    assert_eq!(playback.active_clips(track_id).len(), 0);
}

#[test]
fn arrangement_playback_seek() {
    let mut project = Project::new("Test");
    let track_id = project.add_track(TrackType::Midi, "Track 1");
    
    let clip = MidiClip::new("Clip 1", Beats(4.0));
    let clip_id = project.add_clip(Clip::Midi(clip));
    project.place_clip(track_id, clip_id, Beats(0.0)).unwrap();
    
    let mut playback = ArrangementPlayback::new();
    
    // Start clip
    playback.update(&project, Beats(0.0));
    assert_eq!(playback.active_clips(track_id).len(), 1);
    
    // Seek to position 2.0 (within clip)
    playback.seek(&project, Beats(2.0));
    // Should still have active clip
    assert_eq!(playback.active_clips(track_id).len(), 1);
    
    // Seek to position 10.0 (outside clip)
    playback.seek(&project, Beats(10.0));
    // Should have no active clips
    assert_eq!(playback.active_clips(track_id).len(), 0);
}

#[test]
fn arrangement_playback_stop() {
    let mut project = Project::new("Test");
    let track_id = project.add_track(TrackType::Midi, "Track 1");
    
    let clip = MidiClip::new("Clip 1", Beats(4.0));
    let clip_id = project.add_clip(Clip::Midi(clip));
    project.place_clip(track_id, clip_id, Beats(0.0)).unwrap();
    
    let mut playback = ArrangementPlayback::new();
    
    // Start clip
    playback.update(&project, Beats(0.0));
    assert_eq!(playback.active_clips(track_id).len(), 1);
    
    // Stop all
    playback.stop();
    assert_eq!(playback.active_clips(track_id).len(), 0);
}

#[test]
fn view_manager_default() {
    let manager = ViewManager::new();
    assert_eq!(manager.mode, ViewMode::Session);
}

#[test]
fn view_manager_switch_to_arrangement() {
    let mut manager = ViewManager::new();
    manager.session.set_slot_state(0, 0, SlotState::Playing);
    
    manager.switch_to_arrangement();
    assert_eq!(manager.mode, ViewMode::Arrangement);
    // Session clips should be stopped
    assert_eq!(manager.session.slot_state(0, 0), SlotState::Stopped);
}

#[test]
fn view_manager_switch_to_session() {
    let mut manager = ViewManager::new();
    manager.switch_to_arrangement();
    assert_eq!(manager.mode, ViewMode::Arrangement);
    
    manager.switch_to_session();
    assert_eq!(manager.mode, ViewMode::Session);
    // Arrangement should be stopped (tested via stop being called)
}

#[test]
fn view_manager_default_impl() {
    let manager = ViewManager::default();
    assert_eq!(manager.mode, ViewMode::Session);
}
