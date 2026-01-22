use std::collections::HashMap;
use crate::ids::{TrackId, ClipId};
use crate::transport::Beats;
use crate::project::Project;

/// An actively playing clip in the arrangement.
#[derive(Debug, Clone)]
pub struct ActiveClip {
    pub clip_id: ClipId,
    pub track_id: TrackId,
    pub start_position: Beats,
    pub end_position: Beats,
    pub playback_offset: Beats,
}

/// Manages arrangement playback.
#[derive(Debug, Default)]
pub struct ArrangementPlayback {
    active_clips: HashMap<TrackId, Vec<ActiveClip>>,
    last_position: Beats,
}

impl ArrangementPlayback {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update playback state for the current position.
    pub fn update(&mut self, project: &Project, position: Beats) -> Vec<ArrangementEvent> {
        let mut events = Vec::new();

        for track in project.tracks() {
            let track_clips = self.active_clips.entry(track.id).or_default();

            // Check for clips that should start
            for arr_clip in &track.arrangement_clips {
                if arr_clip.muted {
                    continue;
                }

                let clip_end = Beats(arr_clip.position.0 + arr_clip.length.0);

                // Clip should be playing if we're within its range
                let should_play = position.0 >= arr_clip.position.0 && position.0 < clip_end.0;
                let is_playing = track_clips.iter().any(|c| c.clip_id == arr_clip.clip_id);

                if should_play && !is_playing {
                    // Start the clip
                    track_clips.push(ActiveClip {
                        clip_id: arr_clip.clip_id,
                        track_id: track.id,
                        start_position: arr_clip.position,
                        end_position: clip_end,
                        playback_offset: Beats(position.0 - arr_clip.position.0 + arr_clip.offset.0),
                    });
                    events.push(ArrangementEvent::ClipStarted {
                        track: track.id,
                        clip: arr_clip.clip_id,
                    });
                } else if !should_play && is_playing {
                    // Stop the clip
                    track_clips.retain(|c| c.clip_id != arr_clip.clip_id);
                    events.push(ArrangementEvent::ClipEnded {
                        track: track.id,
                        clip: arr_clip.clip_id,
                    });
                }
            }
        }

        self.last_position = position;
        events
    }

    /// Get active clips for a track.
    pub fn active_clips(&self, track_id: TrackId) -> &[ActiveClip] {
        self.active_clips.get(&track_id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Handle seek (reset all clip states).
    pub fn seek(&mut self, project: &Project, position: Beats) {
        self.active_clips.clear();
        self.last_position = position;
        // Re-evaluate what should be playing
        self.update(project, position);
    }

    /// Stop all clips.
    pub fn stop(&mut self) {
        self.active_clips.clear();
    }
}

/// Events from arrangement playback.
#[derive(Debug, Clone)]
pub enum ArrangementEvent {
    ClipStarted { track: TrackId, clip: ClipId },
    ClipEnded { track: TrackId, clip: ClipId },
}
