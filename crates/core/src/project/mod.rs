//! Project model (tracks, clips, scenes).

mod clip;
mod track;
mod project;

pub use clip::{Clip, MidiClip, AudioClip, ClipHeader, WarpMarker};
pub use track::{Track, TrackType, ArrangementClip};
pub use project::{Project, ProjectMeta, Scene};
