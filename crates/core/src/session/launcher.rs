//! Session-view clip launcher.
//!
//! Owns slot states (`Empty / Stopped / Queued / Playing`), the launch queue,
//! and — as of Slice 9 — the per-slot **playback snapshot** (`ClipPlayback`)
//! that the engine replays on the audio thread.
//!
//! The launcher itself does not touch the audio graph: it tracks what *should*
//! be playing and dequeues queued starts when the transport reaches their
//! quantize boundary. The engine pulls events out of [`ClipLauncher::advance`]
//! and routes them into synth nodes.

use std::collections::HashMap;
use crate::ids::NodeId;
use crate::midi::MidiEvent;
use crate::transport::{Beats, SampleTime};

/// State of a clip slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SlotState {
    #[default]
    Empty,
    Stopped,
    Queued,
    Playing,
    Recording,
}

/// Quantization for clip launching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LaunchQuantize {
    None,
    #[default]
    Bar,
    Beat,
    HalfBeat,
}

impl LaunchQuantize {
    /// Get next quantized position. With [`Self::None`] the result is just
    /// `current` so launch fires immediately.
    pub fn next_position(&self, current: Beats, time_sig_numerator: u8) -> Beats {
        match self {
            Self::None => current,
            Self::Bar => {
                let bar_length = time_sig_numerator as f64;
                let next_bar = (current.0 / bar_length).ceil() * bar_length;
                Beats(next_bar.max(current.0))
            }
            Self::Beat => Beats(current.0.ceil().max(current.0)),
            Self::HalfBeat => Beats(((current.0 * 2.0).ceil() / 2.0).max(current.0)),
        }
    }
}

/// A note baked for clip playback. Times are clip-local (beats from clip start).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipNote {
    /// Clip-local start time, in beats.
    pub start_beats: f64,
    /// Note duration, in beats. Clamped against the clip length when emitted.
    pub length_beats: f64,
    /// MIDI pitch (0-127).
    pub pitch: u8,
    /// Note-on velocity (0-127).
    pub velocity: u8,
    /// MIDI channel (0-15).
    pub channel: u8,
}

/// Snapshot of a clip prepared for engine playback.
///
/// The UI side builds this when the user launches a slot, then sends it via
/// [`Command::LaunchClip`](crate::command::Command::LaunchClip). The engine
/// stores it on its launcher; live edits don't propagate to in-flight playback
/// (re-launch picks up the new notes).
#[derive(Debug, Clone, PartialEq)]
pub struct ClipPlayback {
    /// Synth node to receive the events.
    pub target_node: NodeId,
    /// Loop length, in beats. Clip wraps at this point.
    pub length_beats: f64,
    /// Notes, clip-local; order is irrelevant.
    pub notes: Vec<ClipNote>,
}

/// Event emitted by [`ClipLauncher::advance`] for the engine to consume.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineMidiDispatch {
    /// Synth target.
    pub target: NodeId,
    /// Sample offset within the current audio block.
    pub sample_offset: u32,
    /// MIDI event.
    pub event: MidiEvent,
}

/// Event emitted by the launcher whenever a slot state transitions. The host
/// forwards these as `RuntimeEvent::SlotStateChanged` so the UI can light up
/// triangles and queue diamonds without polling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotStateChange {
    pub track: usize,
    pub scene: usize,
    pub state: SlotState,
}

/// State of a clip currently rendering. Kept on the launcher so a single
/// session-view tick can read it and emit MIDI for the block.
#[derive(Debug, Clone)]
struct ActiveClip {
    playback: ClipPlayback,
    /// Absolute beat at which playback began. `pos_in_clip = (now - launch_beat) mod length`.
    launch_beat: f64,
    /// Pitches currently sounding so we can emit NoteOff on stop / re-trigger.
    sounding: Vec<u8>,
}

/// A pending launch waiting for its quantize boundary.
#[derive(Debug, Clone)]
struct QueuedAction {
    track: usize,
    scene: usize,
    trigger_time: Beats,
    playback: ClipPlayback,
}

/// Output of [`ClipLauncher::advance`] — events and slot-state transitions for
/// one audio block.
#[derive(Debug, Default)]
pub struct AdvanceResult {
    /// MIDI events to dispatch into the graph this block.
    pub midi: Vec<EngineMidiDispatch>,
    /// Slot transitions that occurred this block.
    pub state_changes: Vec<SlotStateChange>,
}

/// Output of a launch / stop call — slot state transitions plus any NoteOff
/// dispatches needed to silence sound that was displaced. The engine merges
/// the events into a single block-level dispatch list.
#[derive(Debug, Default)]
pub struct LaunchOutcome {
    /// Slot state transitions caused by this call.
    pub state_changes: Vec<SlotStateChange>,
    /// Immediate NoteOff dispatches at sample offset 0, for displaced clips.
    pub note_offs: Vec<EngineMidiDispatch>,
}

/// Manages clip launching in session view.
#[derive(Debug)]
pub struct ClipLauncher {
    /// (track_index, scene_index) -> state.
    slot_states: HashMap<(usize, usize), SlotState>,
    /// Clips queued to start.
    queued_starts: Vec<QueuedAction>,
    /// Active clips by (track, scene). At most one per track is playing.
    active: HashMap<(usize, usize), ActiveClip>,
    /// Global launch quantization.
    pub quantize: LaunchQuantize,
}

impl ClipLauncher {
    pub fn new() -> Self {
        Self {
            slot_states: HashMap::new(),
            queued_starts: Vec::new(),
            active: HashMap::new(),
            quantize: LaunchQuantize::Bar,
        }
    }

    /// Get slot state.
    pub fn slot_state(&self, track: usize, scene: usize) -> SlotState {
        self.slot_states
            .get(&(track, scene))
            .copied()
            .unwrap_or_default()
    }

    /// Set slot state. Mostly used by the engine for `SlotState::Stopped` /
    /// `Empty` transitions outside the queue/active path.
    pub fn set_slot_state(&mut self, track: usize, scene: usize, state: SlotState) {
        if matches!(state, SlotState::Empty) {
            self.slot_states.remove(&(track, scene));
        } else {
            self.slot_states.insert((track, scene), state);
        }
    }

    /// All `(track, scene) → state` pairs for non-empty slots. Used by the UI
    /// when rebuilding `SessionViewModel` to mirror engine state.
    pub fn iter_states(&self) -> impl Iterator<Item = (usize, usize, SlotState)> + '_ {
        self.slot_states
            .iter()
            .map(|(&(t, s), &state)| (t, s, state))
    }

    /// Queue a clip to launch on the next quantize boundary, attaching the
    /// playback snapshot. Any clip already playing or queued on the same track
    /// is stopped immediately (Ableton-style "one clip per track"); the
    /// displaced clip's sounding pitches are returned as NoteOff dispatches so
    /// the engine can silence them in the next block.
    pub fn launch_clip(
        &mut self,
        track: usize,
        scene: usize,
        playback: ClipPlayback,
        current_pos: Beats,
        time_sig_numerator: u8,
    ) -> LaunchOutcome {
        let trigger_time = self.quantize.next_position(current_pos, time_sig_numerator);
        let mut outcome = LaunchOutcome::default();

        // Stop any other slot on this track that is queued or playing.
        let other_slots: Vec<(usize, usize)> = self
            .slot_states
            .iter()
            .filter(|(&(t, s), state)| {
                t == track
                    && (s != scene)
                    && matches!(*state, SlotState::Queued | SlotState::Playing)
            })
            .map(|(&k, _)| k)
            .collect();
        for key in other_slots {
            self.slot_states.insert(key, SlotState::Stopped);
            outcome.state_changes.push(SlotStateChange {
                track: key.0,
                scene: key.1,
                state: SlotState::Stopped,
            });
            self.queued_starts.retain(|a| !(a.track == key.0 && a.scene == key.1));
            if let Some(active) = self.active.remove(&key) {
                outcome.note_offs.extend(note_offs(&active, 0));
            }
        }

        // Queue the new clip. Replace any existing queue for this slot.
        self.queued_starts
            .retain(|a| !(a.track == track && a.scene == scene));
        self.queued_starts.push(QueuedAction {
            track,
            scene,
            trigger_time,
            playback,
        });
        self.slot_states.insert((track, scene), SlotState::Queued);
        outcome.state_changes.push(SlotStateChange {
            track,
            scene,
            state: SlotState::Queued,
        });
        outcome
    }

    /// Stop everything on this track: cancel any queue, mark playing slots as
    /// stopped, and drain NoteOffs for sounding pitches.
    pub fn stop_track(&mut self, track: usize) -> LaunchOutcome {
        let mut outcome = LaunchOutcome::default();
        self.queued_starts.retain(|a| {
            if a.track == track {
                outcome.state_changes.push(SlotStateChange {
                    track: a.track,
                    scene: a.scene,
                    state: SlotState::Stopped,
                });
                false
            } else {
                true
            }
        });
        let stopping: Vec<(usize, usize)> = self
            .slot_states
            .iter()
            .filter(|(&(t, _), state)| {
                t == track && matches!(*state, SlotState::Playing | SlotState::Queued)
            })
            .map(|(&k, _)| k)
            .collect();
        for key in stopping {
            self.slot_states.insert(key, SlotState::Stopped);
            if !outcome
                .state_changes
                .iter()
                .any(|c| c.track == key.0 && c.scene == key.1)
            {
                outcome.state_changes.push(SlotStateChange {
                    track: key.0,
                    scene: key.1,
                    state: SlotState::Stopped,
                });
            }
            if let Some(active) = self.active.remove(&key) {
                outcome.note_offs.extend(note_offs(&active, 0));
            }
        }
        outcome
    }

    /// Stop every track. Mirror of [`Self::stop_track`] across the grid.
    pub fn stop_all(&mut self) -> LaunchOutcome {
        let mut outcome = LaunchOutcome::default();
        for action in self.queued_starts.drain(..) {
            outcome.state_changes.push(SlotStateChange {
                track: action.track,
                scene: action.scene,
                state: SlotState::Stopped,
            });
        }
        let stopping: Vec<(usize, usize)> = self
            .slot_states
            .iter()
            .filter(|(_, state)| matches!(**state, SlotState::Playing | SlotState::Queued))
            .map(|(&k, _)| k)
            .collect();
        for key in stopping {
            self.slot_states.insert(key, SlotState::Stopped);
            if !outcome
                .state_changes
                .iter()
                .any(|c| c.track == key.0 && c.scene == key.1)
            {
                outcome.state_changes.push(SlotStateChange {
                    track: key.0,
                    scene: key.1,
                    state: SlotState::Stopped,
                });
            }
        }
        for (_, active) in self.active.drain() {
            outcome.note_offs.extend(note_offs(&active, 0));
        }
        outcome
    }

    /// Advance the launcher across one audio block.
    ///
    /// `block_start_beats` is the transport position at the start of the block;
    /// `block_end_beats` is the position one block later. The launcher walks
    /// any queued starts whose `trigger_time` falls in `[block_start, block_end)`,
    /// activates them at the correct sample offset, then emits the per-block
    /// MIDI events for every active clip.
    pub fn advance(
        &mut self,
        block_start_beats: Beats,
        block_end_beats: Beats,
        tempo_bpm: f64,
        sample_rate: u32,
        block_frames: u32,
    ) -> AdvanceResult {
        let mut result = AdvanceResult::default();

        // 1. Activate any queued clips whose trigger time fell in this block.
        let block_start = block_start_beats.0;
        let block_end = block_end_beats.0;
        let mut still_queued = Vec::with_capacity(self.queued_starts.len());
        for action in self.queued_starts.drain(..) {
            let trigger = action.trigger_time.0;
            // Anything stale (trigger before block start) was probably missed
            // due to UI latency — fire it at offset 0 rather than dropping.
            if trigger < block_end {
                let key = (action.track, action.scene);
                // Slot state may have been flipped to Stopped by a competing
                // launch_clip on the same track. Honor that — drop the start.
                if !matches!(self.slot_states.get(&key), Some(SlotState::Queued)) {
                    continue;
                }
                self.slot_states.insert(key, SlotState::Playing);
                result.state_changes.push(SlotStateChange {
                    track: action.track,
                    scene: action.scene,
                    state: SlotState::Playing,
                });
                self.active.insert(
                    key,
                    ActiveClip {
                        playback: action.playback,
                        launch_beat: trigger.max(block_start),
                        sounding: Vec::new(),
                    },
                );
            } else {
                still_queued.push(action);
            }
        }
        self.queued_starts = still_queued;

        // 2. Emit MIDI events for every active clip across this block.
        if self.active.is_empty() {
            return result;
        }

        for clip in self.active.values_mut() {
            emit_clip_block(
                clip,
                block_start,
                block_end,
                tempo_bpm,
                sample_rate,
                block_frames,
                &mut result.midi,
            );
        }

        result
    }
}

/// Emit MIDI for a single active clip across `[block_start_beats, block_end_beats)`.
fn emit_clip_block(
    clip: &mut ActiveClip,
    block_start_beats: f64,
    block_end_beats: f64,
    tempo_bpm: f64,
    sample_rate: u32,
    block_frames: u32,
    out: &mut Vec<EngineMidiDispatch>,
) {
    let length = clip.playback.length_beats;
    if length <= 0.0 {
        return;
    }

    // Block-relative beat range, clipped to "after the launch beat".
    let start = block_start_beats.max(clip.launch_beat);
    let end = block_end_beats.max(start);
    if end <= start {
        return;
    }

    // Loop iterations whose timeline intersects [start, end). For typical
    // block sizes / clip lengths this is exactly one iteration; only stretches
    // longer than a clip span more.
    let l_min = ((start - clip.launch_beat) / length).floor() as i64;
    let l_max = ((end - clip.launch_beat) / length).floor() as i64;
    let l_max = l_max.max(l_min);

    for l in l_min..=l_max {
        let iter_base = clip.launch_beat + l as f64 * length;
        // Snapshot notes vector locally to avoid borrowing clip.playback while we mutate clip.sounding.
        // (We only read; .clone() of the Vec<ClipNote> would allocate, so iterate by index.)
        let count = clip.playback.notes.len();
        for i in 0..count {
            let note = clip.playback.notes[i];
            let on_beat = iter_base + note.start_beats;
            let off_beat_unclamped = iter_base + (note.start_beats + note.length_beats);
            // Clamp the note off to clip length so notes never bleed past the loop.
            let off_beat = off_beat_unclamped.min(iter_base + length);

            // NoteOn: only if it falls in the block AND is at-or-after the launch.
            if on_beat >= start && on_beat < end {
                let raw_offset = beats_to_offset(on_beat - block_start_beats, tempo_bpm, sample_rate);
                let offset = raw_offset.min(block_frames.saturating_sub(1));
                // If this pitch is already sounding, kill it one sample early
                // so the synth re-triggers cleanly across loop wraps.
                if clip.sounding.contains(&note.pitch) {
                    let pre_off_offset = offset.saturating_sub(1);
                    out.push(EngineMidiDispatch {
                        target: clip.playback.target_node,
                        sample_offset: pre_off_offset,
                        event: midi_note_off(note.channel, note.pitch),
                    });
                    clip.sounding.retain(|&p| p != note.pitch);
                }
                out.push(EngineMidiDispatch {
                    target: clip.playback.target_node,
                    sample_offset: offset,
                    event: midi_note_on(note.channel, note.pitch, note.velocity),
                });
                clip.sounding.push(note.pitch);
            }

            // NoteOff: emit if it falls in this block. Clamping above means
            // it's never outside the loop iteration's beat span.
            if off_beat >= start && off_beat < end {
                let raw_offset = beats_to_offset(off_beat - block_start_beats, tempo_bpm, sample_rate);
                let offset = raw_offset.min(block_frames.saturating_sub(1));
                out.push(EngineMidiDispatch {
                    target: clip.playback.target_node,
                    sample_offset: offset,
                    event: midi_note_off(note.channel, note.pitch),
                });
                clip.sounding.retain(|&p| p != note.pitch);
            }
        }
    }
}

/// Convert a beats delta to a positive sample offset.
fn beats_to_offset(delta_beats: f64, tempo_bpm: f64, sample_rate: u32) -> u32 {
    if delta_beats <= 0.0 {
        return 0;
    }
    SampleTime::from_beats(Beats(delta_beats), tempo_bpm, sample_rate).0 as u32
}

fn midi_note_on(channel: u8, pitch: u8, velocity: u8) -> MidiEvent {
    MidiEvent::NoteOn {
        channel: crate::midi::Channel::new(channel.min(15)).expect("channel <= 15"),
        note: crate::midi::Note::new(pitch.min(127)).expect("note <= 127"),
        velocity: crate::midi::Velocity::new(velocity.min(127)).expect("velocity <= 127"),
    }
}

fn midi_note_off(channel: u8, pitch: u8) -> MidiEvent {
    MidiEvent::note_off(
        crate::midi::Channel::new(channel.min(15)).expect("channel <= 15"),
        crate::midi::Note::new(pitch.min(127)).expect("note <= 127"),
    )
}

fn note_offs(clip: &ActiveClip, sample_offset: u32) -> Vec<EngineMidiDispatch> {
    clip.sounding
        .iter()
        .map(|&pitch| EngineMidiDispatch {
            target: clip.playback.target_node,
            sample_offset,
            event: midi_note_off(0, pitch),
        })
        .collect()
}

impl Default for ClipLauncher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::NodeId;

    fn pb(target: NodeId, length: f64, notes: Vec<(f64, f64, u8)>) -> ClipPlayback {
        ClipPlayback {
            target_node: target,
            length_beats: length,
            notes: notes
                .into_iter()
                .map(|(s, l, p)| ClipNote {
                    start_beats: s,
                    length_beats: l,
                    pitch: p,
                    velocity: 100,
                    channel: 0,
                })
                .collect(),
        }
    }

    #[test]
    fn launch_clip_queues_with_quantize() {
        let mut l = ClipLauncher::new();
        let target = NodeId::generate();
        let outcome = l.launch_clip(0, 0, pb(target, 4.0, vec![]), Beats(1.5), 4);
        assert!(outcome
            .state_changes
            .iter()
            .any(|c| matches!(c.state, SlotState::Queued)));
        assert_eq!(l.slot_state(0, 0), SlotState::Queued);
    }

    #[test]
    fn quantize_none_fires_immediately() {
        let mut l = ClipLauncher::new();
        l.quantize = LaunchQuantize::None;
        let target = NodeId::generate();
        l.launch_clip(0, 0, pb(target, 4.0, vec![(0.0, 1.0, 60)]), Beats(0.0), 4);
        let res = l.advance(Beats(0.0), Beats(0.5), 120.0, 44100, 256);
        assert_eq!(l.slot_state(0, 0), SlotState::Playing);
        assert!(res
            .state_changes
            .iter()
            .any(|c| matches!(c.state, SlotState::Playing)));
        // NoteOn at offset 0.
        assert!(res
            .midi
            .iter()
            .any(|d| d.sample_offset == 0
                && matches!(d.event, MidiEvent::NoteOn { .. })));
    }

    #[test]
    fn note_on_then_off_fires_within_block() {
        let mut l = ClipLauncher::new();
        l.quantize = LaunchQuantize::None;
        let target = NodeId::generate();
        // 4-beat clip with one note: starts at 0.0, lasts 0.5 beats.
        l.launch_clip(0, 0, pb(target, 4.0, vec![(0.0, 0.5, 60)]), Beats(0.0), 4);
        // Block long enough to span both NoteOn and NoteOff: 1 beat at 120 BPM = 22050 frames.
        let res = l.advance(Beats(0.0), Beats(1.0), 120.0, 44100, 22050);
        let on_count = res
            .midi
            .iter()
            .filter(|d| matches!(d.event, MidiEvent::NoteOn { .. }))
            .count();
        let off_count = res
            .midi
            .iter()
            .filter(|d| matches!(d.event, MidiEvent::NoteOff { .. }))
            .count();
        assert_eq!(on_count, 1);
        assert_eq!(off_count, 1);
    }

    #[test]
    fn loop_wrap_retriggers_first_note() {
        let mut l = ClipLauncher::new();
        l.quantize = LaunchQuantize::None;
        let target = NodeId::generate();
        // 1-beat clip with note at start.
        l.launch_clip(0, 0, pb(target, 1.0, vec![(0.0, 0.5, 60)]), Beats(0.0), 4);
        // Two full loops: 2 beats at 120 BPM = 44100 frames.
        let res = l.advance(Beats(0.0), Beats(2.0), 120.0, 44100, 44100);
        let on_count = res
            .midi
            .iter()
            .filter(|d| matches!(d.event, MidiEvent::NoteOn { .. }))
            .count();
        assert!(on_count >= 2, "expected ≥2 NoteOns across two loops, got {}", on_count);
    }

    #[test]
    fn stop_track_drains_active_note_offs() {
        let mut l = ClipLauncher::new();
        l.quantize = LaunchQuantize::None;
        let target = NodeId::generate();
        // Clip with a note holding through the whole length.
        l.launch_clip(0, 0, pb(target, 4.0, vec![(0.0, 4.0, 60)]), Beats(0.0), 4);
        // Quarter-block — NoteOn fires, NoteOff does not yet.
        l.advance(Beats(0.0), Beats(0.5), 120.0, 44100, 11025);
        // Stop while the note is still sounding.
        let outcome = l.stop_track(0);
        assert_eq!(outcome.note_offs.len(), 1);
        assert!(matches!(outcome.note_offs[0].event, MidiEvent::NoteOff { .. }));
    }

    #[test]
    fn launching_second_slot_on_track_stops_first() {
        let mut l = ClipLauncher::new();
        let target = NodeId::generate();
        l.launch_clip(0, 0, pb(target, 4.0, vec![]), Beats(0.0), 4);
        let outcome = l.launch_clip(0, 1, pb(target, 4.0, vec![]), Beats(0.0), 4);
        // The earlier slot 0 should have been transitioned to Stopped.
        assert!(outcome
            .state_changes
            .iter()
            .any(|c| c.scene == 0 && matches!(c.state, SlotState::Stopped)));
        assert_eq!(l.slot_state(0, 0), SlotState::Stopped);
        assert_eq!(l.slot_state(0, 1), SlotState::Queued);
    }

    #[test]
    fn displacing_active_clip_drains_note_offs() {
        let mut l = ClipLauncher::new();
        l.quantize = LaunchQuantize::None;
        let target = NodeId::generate();
        // Hold a long note on slot 0.
        l.launch_clip(0, 0, pb(target, 4.0, vec![(0.0, 4.0, 60)]), Beats(0.0), 4);
        l.advance(Beats(0.0), Beats(0.5), 120.0, 44100, 11025);
        // Launch a different slot on the same track — old clip should be drained.
        let outcome = l.launch_clip(0, 1, pb(target, 4.0, vec![]), Beats(0.5), 4);
        assert!(
            outcome
                .note_offs
                .iter()
                .any(|d| matches!(d.event, MidiEvent::NoteOff { .. })),
            "displacing slot must drain NoteOffs from the previously sounding clip"
        );
    }

    #[test]
    fn launch_quantize_bar_snaps_to_next_bar() {
        let mut l = ClipLauncher::new();
        let target = NodeId::generate();
        l.launch_clip(0, 0, pb(target, 4.0, vec![]), Beats(2.5), 4);
        // Trigger should be at beat 4.0.
        l.advance(Beats(2.5), Beats(3.5), 120.0, 44100, 22050);
        assert_eq!(l.slot_state(0, 0), SlotState::Queued);
        let res = l.advance(Beats(3.5), Beats(4.5), 120.0, 44100, 22050);
        assert_eq!(l.slot_state(0, 0), SlotState::Playing);
        assert!(res
            .state_changes
            .iter()
            .any(|c| matches!(c.state, SlotState::Playing)));
    }
}
