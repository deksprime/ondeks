//! Piano roll view model for MIDI editing.

use ondeks_core::ClipId;
use ondeks_core::transport::Beats;
use ondeks_core::midi::{MidiEvent, TimestampedEvent};
use ondeks_core::project::MidiClip;
use crate::types::{TimeRange, GridSettings};

/// A MIDI note for display in the piano roll.
#[derive(Debug, Clone)]
pub struct NoteViewModel {
    /// Index in the clip's sequence
    pub index: usize,
    /// MIDI note number (0-127)
    pub pitch: u8,
    /// Start position in beats
    pub start: f64,
    /// Duration in beats
    pub duration: f64,
    /// Velocity (0-127)
    pub velocity: u8,
    /// Is this note selected
    pub is_selected: bool,
    /// Is this note currently playing
    pub is_playing: bool,
}

impl NoteViewModel {
    /// Get end position in beats.
    pub fn end(&self) -> f64 {
        self.start + self.duration
    }

    /// Get note name (e.g., "C4", "F#3").
    pub fn note_name(&self) -> String {
        note_to_name(self.pitch)
    }

    /// Check if note overlaps a time range.
    pub fn overlaps(&self, range: &TimeRange) -> bool {
        self.start < range.end.0 && self.end() > range.start.0
    }
}

/// Get note name from MIDI number.
fn note_to_name(midi_note: u8) -> String {
    const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let octave = (midi_note / 12) as i32 - 1;
    let note = NOTE_NAMES[(midi_note % 12) as usize];
    format!("{}{}", note, octave)
}

/// Get MIDI number from note name.
pub fn name_to_note(name: &str) -> Option<u8> {
    let name = name.to_uppercase();
    let (note_part, octave_part) = if name.contains('#') || name.contains('B') && name.len() > 2 {
        name.split_at(2)
    } else {
        name.split_at(1)
    };

    let note_offset = match note_part {
        "C" => 0, "C#" | "DB" => 1,
        "D" => 2, "D#" | "EB" => 3,
        "E" => 4,
        "F" => 5, "F#" | "GB" => 6,
        "G" => 7, "G#" | "AB" => 8,
        "A" => 9, "A#" | "BB" => 10,
        "B" => 11,
        _ => return None,
    };

    let octave: i32 = octave_part.parse().ok()?;
    let midi = (octave + 1) * 12 + note_offset;

    if midi >= 0 && midi <= 127 {
        Some(midi as u8)
    } else {
        None
    }
}

/// Piano roll view model.
#[derive(Debug, Clone)]
pub struct PianoRollViewModel {
    /// Clip being edited
    pub clip_id: ClipId,
    /// Clip name
    pub clip_name: String,
    /// Clip length in beats
    pub clip_length: f64,
    /// All notes in the clip
    pub notes: Vec<NoteViewModel>,
    /// Current zoom level
    pub zoom: PianoRollZoom,
    /// Current scroll position
    pub scroll: PianoRollScroll,
    /// Grid settings
    pub grid: GridSettings,
    /// Visible pitch range
    pub visible_pitch_range: (u8, u8),
    /// Visible time range
    pub visible_time_range: TimeRange,
    /// Current playhead position (relative to clip)
    pub playhead: Option<f64>,
    /// Currently held preview note
    pub preview_note: Option<u8>,
}

/// Zoom settings for piano roll.
#[derive(Debug, Clone, Copy)]
pub struct PianoRollZoom {
    /// Pixels per beat (horizontal)
    pub pixels_per_beat: f32,
    /// Pixels per semitone (vertical)
    pub pixels_per_semitone: f32,
}

impl Default for PianoRollZoom {
    fn default() -> Self {
        Self {
            pixels_per_beat: 80.0,
            pixels_per_semitone: 16.0,
        }
    }
}

/// Scroll settings for piano roll.
#[derive(Debug, Clone, Copy, Default)]
pub struct PianoRollScroll {
    /// Beat offset (horizontal scroll)
    pub beat_offset: f64,
    /// Lowest visible MIDI note
    pub lowest_note: u8,
}

impl PianoRollViewModel {
    /// Create from a MIDI clip.
    pub fn from_clip(
        clip: &MidiClip,
        is_note_selected: impl Fn(usize) -> bool,
        playing_notes: &[u8],
        zoom: PianoRollZoom,
        scroll: PianoRollScroll,
        grid: GridSettings,
    ) -> Self {
        let mut notes = Vec::new();
        let mut note_index = 0;

        // Extract notes from sequence
        for event in clip.sequence.events() {
            if let MidiEvent::NoteOn { note, velocity, .. } = &event.event {
                // Find matching note-off to get duration
                let duration = find_note_duration(&clip.sequence, event.time, note.raw());

                notes.push(NoteViewModel {
                    index: note_index,
                    pitch: note.raw(),
                    start: event.time.0,
                    duration,
                    velocity: velocity.raw(),
                    is_selected: is_note_selected(note_index),
                    is_playing: playing_notes.contains(&note.raw()),
                });
                note_index += 1;
            }
        }

        // Calculate visible ranges
        let (lowest, highest) = if notes.is_empty() {
            (48, 72) // Default C3 to C5
        } else {
            let lowest = notes.iter().map(|n| n.pitch).min().unwrap_or(60);
            let highest = notes.iter().map(|n| n.pitch).max().unwrap_or(72);
            (lowest.saturating_sub(6), highest.saturating_add(6).min(127))
        };

        Self {
            clip_id: clip.id(),
            clip_name: clip.header.name.clone(),
            clip_length: clip.header.length.0,
            notes,
            zoom,
            scroll,
            grid,
            visible_pitch_range: (scroll.lowest_note, scroll.lowest_note + 24), // ~2 octaves visible
            visible_time_range: TimeRange::new(
                Beats(scroll.beat_offset),
                Beats(scroll.beat_offset + 8.0), // Placeholder
            ),
            playhead: None,
            preview_note: None,
        }
    }

    /// Get notes visible in the current viewport.
    pub fn visible_notes(&self) -> impl Iterator<Item = &NoteViewModel> {
        self.notes.iter().filter(|n| {
            n.pitch >= self.visible_pitch_range.0
            && n.pitch <= self.visible_pitch_range.1
            && n.overlaps(&self.visible_time_range)
        })
    }

    /// Convert beat position to pixel X.
    pub fn beat_to_x(&self, beat: f64) -> f32 {
        ((beat - self.scroll.beat_offset) * self.zoom.pixels_per_beat as f64) as f32
    }

    /// Convert pixel X to beat position.
    pub fn x_to_beat(&self, x: f32) -> f64 {
        (x as f64 / self.zoom.pixels_per_beat as f64) + self.scroll.beat_offset
    }

    /// Convert MIDI note to pixel Y (note: higher pitches = lower Y).
    pub fn note_to_y(&self, note: u8) -> f32 {
        (self.visible_pitch_range.1 as f32 - note as f32) * self.zoom.pixels_per_semitone
    }

    /// Convert pixel Y to MIDI note.
    pub fn y_to_note(&self, y: f32) -> u8 {
        let note = self.visible_pitch_range.1 as f32 - (y / self.zoom.pixels_per_semitone);
        (note.round() as u8).clamp(0, 127)
    }

    /// Get notes at a specific pitch.
    pub fn notes_at_pitch(&self, pitch: u8) -> impl Iterator<Item = &NoteViewModel> {
        self.notes.iter().filter(move |n| n.pitch == pitch)
    }

    /// Check if a pitch is a black key.
    pub fn is_black_key(pitch: u8) -> bool {
        matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
    }
}

/// Find the duration of a note by looking for its note-off event.
fn find_note_duration(sequence: &ondeks_core::midi::MidiSequence, start: Beats, pitch: u8) -> f64 {
    for event in sequence.events() {
        if event.time.0 > start.0 {
            if let MidiEvent::NoteOff { note, .. } = &event.event {
                if note.raw() == pitch {
                    return event.time.0 - start.0;
                }
            }
        }
    }
    // Default to 1 beat if no note-off found
    1.0
}

/// Velocity lane view model (for velocity editing below piano roll).
#[derive(Debug, Clone)]
pub struct VelocityLaneViewModel {
    /// Notes with their velocities
    pub velocities: Vec<(usize, f64, u8)>, // (index, position, velocity)
}

impl VelocityLaneViewModel {
    pub fn from_notes(notes: &[NoteViewModel]) -> Self {
        Self {
            velocities: notes.iter()
                .map(|n| (n.index, n.start, n.velocity))
                .collect(),
        }
    }
}
