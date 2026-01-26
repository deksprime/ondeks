//! Mixer view model.

use ondeks_core::TrackId;
use ondeks_core::project::{Project, TrackType};
use super::project::TrackViewModel;
use super::transport::MeterViewModel;

/// A send routing for display.
#[derive(Debug, Clone)]
pub struct SendViewModel {
    /// Source track ID
    pub source_track: TrackId,
    /// Destination (return) track ID
    pub destination_track: TrackId,
    /// Send level in dB
    pub level_db: f32,
    /// Destination track name
    pub destination_name: String,
    /// Is the send enabled (level > -inf)
    pub is_active: bool,
}

impl SendViewModel {
    pub fn level_string(&self) -> String {
        if self.level_db <= -60.0 {
            "-∞".to_string()
        } else {
            format!("{:+.1}", self.level_db)
        }
    }
}

/// A single channel strip in the mixer.
#[derive(Debug, Clone)]
pub struct ChannelStripViewModel {
    /// Track info
    pub track: TrackViewModel,
    /// Current meter readings
    pub meters: MeterViewModel,
    /// Send routings from this track
    pub sends: Vec<SendViewModel>,
    /// Is this strip selected
    pub is_selected: bool,
    /// Is this the master strip
    pub is_master: bool,
    /// Effective mute state (considering solo)
    pub effectively_muted: bool,
}

impl ChannelStripViewModel {
    /// Get the meter bar height for left channel (0.0 to 1.0).
    pub fn left_meter_height(&self) -> f32 {
        self.meters.left_normalized().min(1.2)
    }

    /// Get the meter bar height for right channel (0.0 to 1.0).
    pub fn right_meter_height(&self) -> f32 {
        self.meters.right_normalized().min(1.2)
    }
}

/// Complete mixer view model.
#[derive(Debug, Clone)]
pub struct MixerViewModel {
    /// Regular track channel strips (Audio, MIDI, Group)
    pub tracks: Vec<ChannelStripViewModel>,
    /// Return track channel strips
    pub returns: Vec<ChannelStripViewModel>,
    /// Master channel strip
    pub master: Option<ChannelStripViewModel>,
    /// Is any track soloed
    pub any_soloed: bool,
    /// Master output meters
    pub master_meters: MeterViewModel,
}

impl MixerViewModel {
    /// Build mixer view from project.
    pub fn from_project(
        project: &Project,
        get_meters: impl Fn(TrackId) -> MeterViewModel,
        selected_track: Option<TrackId>,
    ) -> Self {
        let any_soloed = project.tracks().iter().any(|t| t.soloed);

        let mut tracks = Vec::new();
        let mut returns = Vec::new();
        let mut master = None;

        for (idx, track) in project.tracks().iter().enumerate() {
            let track_vm = TrackViewModel::from_track(track, idx);
            let meters = get_meters(track.id);
            let is_selected = selected_track == Some(track.id);

            // Build sends
            let sends: Vec<SendViewModel> = track.sends.iter()
                .filter_map(|(dest_id, &level)| {
                    project.get_track(*dest_id).map(|dest| SendViewModel {
                        source_track: track.id,
                        destination_track: *dest_id,
                        level_db: level,
                        destination_name: dest.name.clone(),
                        is_active: level > -60.0,
                    })
                })
                .collect();

            // Calculate effective mute state
            let effectively_muted = track.muted || (any_soloed && !track.soloed);

            let strip = ChannelStripViewModel {
                track: track_vm,
                meters,
                sends,
                is_selected,
                is_master: track.track_type == TrackType::Master,
                effectively_muted,
            };

            match track.track_type {
                TrackType::Master => master = Some(strip),
                TrackType::Return => returns.push(strip),
                _ => tracks.push(strip),
            }
        }

        let master_meters = master.as_ref()
            .map(|m| m.meters.clone())
            .unwrap_or_default();

        Self {
            tracks,
            returns,
            master,
            any_soloed,
            master_meters,
        }
    }

    /// Get total number of visible strips.
    pub fn strip_count(&self) -> usize {
        self.tracks.len() + self.returns.len() + if self.master.is_some() { 1 } else { 0 }
    }

    /// Get all strips in display order.
    pub fn all_strips(&self) -> impl Iterator<Item = &ChannelStripViewModel> {
        self.tracks.iter()
            .chain(self.returns.iter())
            .chain(self.master.iter())
    }
}
