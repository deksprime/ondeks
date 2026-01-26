//! Arrangement view model (timeline).

use ondeks_core::{TrackId, ClipId, Color};
use ondeks_core::project::{Project, Clip, ArrangementClip};
use ondeks_core::transport::Beats;
use ondeks_core::automation::{AutomationLane, AutomationTarget};
use super::project::{TrackViewModel};
use crate::types::{ZoomLevel, ScrollPosition, TimeRange};

/// A clip as placed on the arrangement timeline.
#[derive(Debug, Clone)]
pub struct ArrangementClipViewModel {
    /// Clip ID
    pub clip_id: ClipId,
    /// Track ID containing this clip
    pub track_id: TrackId,
    /// Track index for rendering
    pub track_index: usize,
    /// Start position in beats
    pub position: f64,
    /// Length in beats
    pub length: f64,
    /// Offset into the clip (for trimmed clips)
    pub offset: f64,
    /// Clip name
    pub name: String,
    /// Clip color
    pub color: Color,
    /// Is this an audio or MIDI clip
    pub is_midi: bool,
    /// Is the clip muted
    pub muted: bool,
    /// Is this clip selected
    pub is_selected: bool,
}

impl ArrangementClipViewModel {
    pub fn from_arrangement_clip(
        arr_clip: &ArrangementClip,
        clip: &Clip,
        track_id: TrackId,
        track_index: usize,
        is_selected: bool,
    ) -> Self {
        Self {
            clip_id: arr_clip.clip_id,
            track_id,
            track_index,
            position: arr_clip.position.0,
            length: arr_clip.length.0,
            offset: arr_clip.offset.0,
            name: clip.header().name.clone(),
            color: clip.header().color,
            is_midi: matches!(clip, Clip::Midi(_)),
            muted: arr_clip.muted,
            is_selected,
        }
    }

    /// Get end position in beats.
    pub fn end_position(&self) -> f64 {
        self.position + self.length
    }

    /// Check if this clip overlaps with a time range.
    pub fn overlaps(&self, range: &TimeRange) -> bool {
        self.position < range.end.0 && self.end_position() > range.start.0
    }
}

/// An automation point for display.
#[derive(Debug, Clone)]
pub struct AutomationPointViewModel {
    pub position: f64,
    pub value: f32,
    pub index: usize,
    pub is_selected: bool,
}

/// An automation lane for display.
#[derive(Debug, Clone)]
pub struct AutomationLaneViewModel {
    pub target: String,
    pub points: Vec<AutomationPointViewModel>,
    pub is_visible: bool,
    pub is_expanded: bool,
    /// Value range for display
    pub min_value: f32,
    pub max_value: f32,
}

impl AutomationLaneViewModel {
    pub fn new(target: &AutomationTarget, lane: &AutomationLane) -> Self {
        let target_string = match target {
            AutomationTarget::TrackVolume(_) => "Volume".to_string(),
            AutomationTarget::TrackPan(_) => "Pan".to_string(),
            AutomationTarget::TrackMute(_) => "Mute".to_string(),
            AutomationTarget::SendLevel { .. } => "Send".to_string(),
            AutomationTarget::PluginParameter { parameter, .. } => {
                format!("Param {:?}", parameter)
            }
        };

        Self {
            target: target_string,
            points: lane.points()
                .iter()
                .enumerate()
                .map(|(i, p)| AutomationPointViewModel {
                    position: p.position.0,
                    value: p.value,
                    index: i,
                    is_selected: false,
                })
                .collect(),
            is_visible: true,
            is_expanded: false,
            min_value: 0.0,
            max_value: 1.0,
        }
    }

    /// Get value at a specific position (interpolated).
    pub fn value_at(&self, position: f64) -> f32 {
        if self.points.is_empty() {
            return 0.5;
        }

        // Find surrounding points
        let mut prev = &self.points[0];
        for point in &self.points {
            if point.position > position {
                // Linear interpolation
                let t = if point.position > prev.position {
                    ((position - prev.position) / (point.position - prev.position)) as f32
                } else {
                    0.0
                };
                return prev.value + t * (point.value - prev.value);
            }
            prev = point;
        }

        // Past all points, return last value
        prev.value
    }
}

/// Complete arrangement view model.
#[derive(Debug, Clone)]
pub struct ArrangementViewModel {
    /// All tracks (for track headers)
    pub tracks: Vec<TrackViewModel>,
    /// All clips placed on the timeline
    pub clips: Vec<ArrangementClipViewModel>,
    /// Automation lanes (per track)
    pub automation: Vec<(TrackId, Vec<AutomationLaneViewModel>)>,
    /// Current zoom level
    pub zoom: ZoomLevel,
    /// Current scroll position
    pub scroll: ScrollPosition,
    /// Visible time range (for culling)
    pub visible_range: TimeRange,
    /// Total arrangement length in beats
    pub total_length: f64,
    /// Loop region (if enabled)
    pub loop_region: Option<TimeRange>,
    /// Current playhead position
    pub playhead: f64,
}

impl ArrangementViewModel {
    /// Build arrangement view from project.
    pub fn from_project(
        project: &Project,
        is_clip_selected: impl Fn(ClipId) -> bool,
        zoom: ZoomLevel,
        scroll: ScrollPosition,
        playhead: f64,
        loop_region: Option<TimeRange>,
    ) -> Self {
        let tracks: Vec<_> = project.tracks()
            .iter()
            .enumerate()
            .map(|(i, t)| TrackViewModel::from_track(t, i))
            .collect();

        // Collect all clips from all tracks
        let mut clips = Vec::new();
        let mut total_length = 0.0f64;

        for (track_idx, track) in project.tracks().iter().enumerate() {
            for arr_clip in &track.arrangement_clips {
                if let Some(clip) = project.get_clip(arr_clip.clip_id) {
                    let is_selected = is_clip_selected(arr_clip.clip_id);
                    clips.push(ArrangementClipViewModel::from_arrangement_clip(
                        arr_clip,
                        clip,
                        track.id,
                        track_idx,
                        is_selected,
                    ));
                    total_length = total_length.max(arr_clip.position.0 + arr_clip.length.0);
                }
            }
        }

        // Ensure minimum length
        total_length = total_length.max(16.0);

        // Calculate visible range from scroll and zoom
        // (This would be refined based on actual viewport size)
        let visible_range = TimeRange::new(
            Beats(scroll.beat_offset),
            Beats(scroll.beat_offset + 32.0), // Placeholder, should use viewport width
        );

        Self {
            tracks,
            clips,
            automation: Vec::new(), // TODO: Add automation lanes
            zoom,
            scroll,
            visible_range,
            total_length,
            loop_region,
            playhead,
        }
    }

    /// Get clips visible in the current viewport.
    pub fn visible_clips(&self) -> impl Iterator<Item = &ArrangementClipViewModel> {
        self.clips.iter().filter(|c| c.overlaps(&self.visible_range))
    }

    /// Get clips on a specific track.
    pub fn clips_on_track(&self, track_index: usize) -> impl Iterator<Item = &ArrangementClipViewModel> {
        self.clips.iter().filter(move |c| c.track_index == track_index)
    }

    /// Convert beat position to pixel X coordinate.
    pub fn beat_to_x(&self, beat: f64) -> f32 {
        ((beat - self.scroll.beat_offset) * self.zoom.pixels_per_beat as f64) as f32
    }

    /// Convert pixel X coordinate to beat position.
    pub fn x_to_beat(&self, x: f32) -> f64 {
        (x as f64 / self.zoom.pixels_per_beat as f64) + self.scroll.beat_offset
    }

    /// Convert track index to pixel Y coordinate.
    pub fn track_to_y(&self, track_index: usize) -> f32 {
        (track_index as f32 - self.scroll.track_offset) * self.zoom.track_height
    }

    /// Convert pixel Y coordinate to track index.
    pub fn y_to_track(&self, y: f32) -> usize {
        ((y / self.zoom.track_height) + self.scroll.track_offset) as usize
    }
}
