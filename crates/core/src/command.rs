//! Commands that can be sent to the audio engine.

use crate::ids::{NodeId, TrackId};
use crate::midi::MidiEvent;
use crate::session::{ClipPlayback, LaunchQuantize};

/// Commands that control the audio engine.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Start playback.
    Play,
    /// Stop playback.
    Stop,
    /// Set the tempo in BPM.
    SetTempo(f64),
    /// Deliver a MIDI event to a specific node's MIDI inbox.
    ///
    /// `sample_offset` is the offset within the current audio block at which
    /// the event should take effect (P0.2, sample-accurate event timing). For
    /// UI-originated events where sub-block precision is meaningless, use 0.
    SendMidi {
        /// The node to receive the event.
        target: NodeId,
        /// The MIDI event to deliver.
        event: MidiEvent,
        /// Sample offset within the current block (0 = block start).
        sample_offset: u32,
    },

    /// Insert a `SynthNode` + `ChannelStripNode` pair into the graph and wire
    /// `Synth → Strip → MasterStrip` (the master strip already exists).
    ///
    /// Both ids are pre-allocated by the UI dispatcher and stored on the
    /// owning `Track::instrument` / `Track::channel_strip` so node identity
    /// is stable across undo/redo cycles.
    AddInstrumentChannel {
        /// Node id assigned to the synth.
        synth_node_id: NodeId,
        /// Node id assigned to the strip.
        strip_node_id: NodeId,
        /// Track this channel belongs to (informational; engine keys by node id).
        track_id: TrackId,
    },
    /// Disconnect and remove a synth + its channel strip.
    ///
    /// No-op if either node is absent.
    RemoveInstrumentChannel {
        /// Node id of the synth.
        synth_node_id: NodeId,
        /// Node id of the strip.
        strip_node_id: NodeId,
    },

    /// Set a track's channel-strip volume in decibels. Smoothed on the
    /// audio side over ~50 ms.
    SetTrackVolume {
        /// Strip node id (the dispatcher reads this from `Track::channel_strip`).
        node_id: NodeId,
        /// New target volume in dB.
        volume_db: f32,
    },
    /// Set a track's pan. -1.0 = full left, 0.0 = center, 1.0 = full right.
    SetTrackPan {
        /// Strip node id.
        node_id: NodeId,
        /// New target pan, clamped to [-1, 1].
        pan: f32,
    },
    /// Toggle/set mute on a strip.
    SetTrackMute {
        /// Strip node id.
        node_id: NodeId,
        /// New mute state.
        muted: bool,
    },
    /// Toggle/set solo on a strip. Engine recomputes the
    /// `silenced_by_other_solo` flag on every other strip.
    SetTrackSolo {
        /// Strip node id.
        node_id: NodeId,
        /// New solo state.
        soloed: bool,
    },
    /// Set the master output volume in decibels.
    SetMasterVolume {
        /// New target master volume in dB.
        volume_db: f32,
    },

    /// Launch a session clip.
    ///
    /// The UI snapshots the clip's notes + the track's instrument node id into
    /// `playback` before sending — the engine never reaches into `Project`. The
    /// launcher quantizes the start to the next bar by default; the clip then
    /// loops at `playback.length_beats` until stopped or replaced.
    LaunchClip {
        /// Track index in the session grid (0-based, master excluded).
        track: usize,
        /// Scene index in the session grid (0-based).
        scene: usize,
        /// Snapshot of the clip prepared for engine playback.
        playback: ClipPlayback,
    },
    /// Stop every clip on a single track. Drains hanging notes.
    StopTrack {
        /// Track index in the session grid.
        track: usize,
    },
    /// Launch every non-empty slot in a scene at the next quantize boundary.
    /// `playbacks` carries one snapshot per non-empty slot.
    LaunchScene {
        /// Scene index.
        scene: usize,
        /// `(track, ClipPlayback)` for every slot to launch.
        playbacks: Vec<(usize, ClipPlayback)>,
    },
    /// Stop every playing or queued clip across the session grid.
    StopAll,
    /// Set the launcher's quantize mode. Tests use `LaunchQuantize::None` to
    /// fire clips immediately; the UI keeps it on `Bar` by default.
    SetLaunchQuantize(LaunchQuantize),
}
