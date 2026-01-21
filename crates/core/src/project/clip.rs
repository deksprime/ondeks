use crate::ids::{ClipId, AudioPoolId, Color};
use crate::transport::Beats;
use crate::midi::MidiSequence;

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

/// A MIDI clip containing a sequence of events.
#[derive(Debug, Clone)]
pub struct MidiClip {
    pub header: ClipHeader,
    pub sequence: MidiSequence,
}

impl MidiClip {
    pub fn new(name: impl Into<String>, length: Beats) -> Self {
        Self {
            header: ClipHeader::new(name, length),
            sequence: MidiSequence::with_length(length),
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
