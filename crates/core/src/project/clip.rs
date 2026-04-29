use crate::ids::{ClipId, AudioPoolId, Color};
use crate::transport::Beats;
use crate::midi::{Channel, MidiSequence, Note, Velocity};

/// Common clip metadata.
#[derive(Debug, Clone)]
pub struct ClipHeader {
    pub id: ClipId,
    pub name: String,
    pub color: Color,
    pub length: Beats,
}

impl ClipHeader {
    pub fn new(name: impl Into<String>, length: Beats) -> Self {
        Self {
            id: ClipId::generate(),
            name: name.into(),
            color: Color::CYAN,
            length,
        }
    }
}

/// A single editable note in a MIDI clip.
///
/// This is the high-level note abstraction the piano roll edits — a
/// rectangle on the grid with start time, length, pitch, velocity. At
/// playback time (Slice 9) each `MidiNote` is lowered into a `NoteOn` /
/// `NoteOff` pair on the synth's MIDI inbox.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MidiNote {
    /// Note start time in beats from the clip start.
    pub time: Beats,
    /// Note length in beats.
    pub length: Beats,
    /// MIDI pitch (0-127).
    pub pitch: Note,
    /// Note-on velocity (0-127).
    pub velocity: Velocity,
    /// MIDI channel (0-15).
    pub channel: Channel,
}

impl MidiNote {
    /// Create a new note. Defaults to channel 0, velocity 100.
    pub fn new(time: Beats, length: Beats, pitch: Note) -> Self {
        Self {
            time,
            length,
            pitch,
            velocity: Velocity::new(100).expect("100 is a valid velocity"),
            channel: Channel::new(0).expect("0 is a valid channel"),
        }
    }
}

/// A MIDI clip — a list of editable notes plus a legacy event sequence.
///
/// `notes` is the primary editable surface (piano roll edits go here).
/// `sequence` survives for control-change / pitch-bend / aftertouch events
/// that don't fit the note rectangle abstraction; for note-only clips it
/// stays empty.
#[derive(Debug, Clone)]
pub struct MidiClip {
    pub header: ClipHeader,
    pub sequence: MidiSequence,
    pub notes: Vec<MidiNote>,
}

impl MidiClip {
    pub fn new(name: impl Into<String>, length: Beats) -> Self {
        Self {
            header: ClipHeader::new(name, length),
            sequence: MidiSequence::with_length(length),
            notes: Vec::new(),
        }
    }

    pub fn id(&self) -> ClipId {
        self.header.id
    }
}

/// A warp marker for time-stretching audio.
#[derive(Debug, Clone)]
pub struct WarpMarker {
    /// Position in the clip (beats).
    pub clip_time: Beats,
    /// Position in the original audio (samples).
    pub sample_time: u64,
}

/// An audio clip referencing the audio pool.
#[derive(Debug, Clone)]
pub struct AudioClip {
    pub header: ClipHeader,
    pub pool_id: AudioPoolId,
    pub warp_markers: Vec<WarpMarker>,
    /// Offset into the audio file (samples).
    pub start_offset: u64,
    /// Gain adjustment in dB.
    pub gain_db: f32,
}

impl AudioClip {
    pub fn new(name: impl Into<String>, pool_id: AudioPoolId, length: Beats) -> Self {
        Self {
            header: ClipHeader::new(name, length),
            pool_id,
            warp_markers: Vec::new(),
            start_offset: 0,
            gain_db: 0.0,
        }
    }

    pub fn id(&self) -> ClipId {
        self.header.id
    }
}

/// A clip (MIDI or Audio).
#[derive(Debug, Clone)]
pub enum Clip {
    Midi(MidiClip),
    Audio(AudioClip),
}

impl Clip {
    pub fn id(&self) -> ClipId {
        match self {
            Self::Midi(c) => c.id(),
            Self::Audio(c) => c.id(),
        }
    }

    pub fn header(&self) -> &ClipHeader {
        match self {
            Self::Midi(c) => &c.header,
            Self::Audio(c) => &c.header,
        }
    }

    pub fn header_mut(&mut self) -> &mut ClipHeader {
        match self {
            Self::Midi(c) => &mut c.header,
            Self::Audio(c) => &mut c.header,
        }
    }

    pub fn length(&self) -> Beats {
        self.header().length
    }
}
