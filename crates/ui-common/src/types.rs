//! Core UI types shared across all interfaces.

use ondeks_core::transport::Beats;

/// Represents a 2D position in the UI (pixels or grid units).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UiPoint {
    pub x: f32,
    pub y: f32,
}

impl UiPoint {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Represents a rectangular region in the UI.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UiRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl UiRect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    pub fn contains(&self, point: UiPoint) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }
}

/// Zoom level for timeline views.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoomLevel {
    /// Pixels per beat (horizontal zoom)
    pub pixels_per_beat: f32,
    /// Pixels per track height (vertical zoom)
    pub track_height: f32,
}

impl Default for ZoomLevel {
    fn default() -> Self {
        Self {
            pixels_per_beat: 50.0,
            track_height: 80.0,
        }
    }
}

/// Scroll position for a view.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ScrollPosition {
    /// Horizontal scroll in beats
    pub beat_offset: f64,
    /// Vertical scroll (track index as float for smooth scrolling)
    pub track_offset: f32,
}

/// A time range for selection or loop regions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeRange {
    pub start: Beats,
    pub end: Beats,
}

impl TimeRange {
    pub fn new(start: Beats, end: Beats) -> Self {
        Self { start, end }
    }

    pub fn duration(&self) -> Beats {
        Beats(self.end.0 - self.start.0)
    }

    pub fn contains(&self, position: Beats) -> bool {
        position.0 >= self.start.0 && position.0 < self.end.0
    }
}

/// Editing tool currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditTool {
    #[default]
    Select,
    Draw,
    Erase,
    Split,
    Stretch,
}

/// Grid snap settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridSettings {
    /// Snap to grid enabled
    pub snap_enabled: bool,
    /// Grid division (1.0 = quarter note, 0.5 = eighth, etc.)
    pub division: f64,
    /// Triplet mode
    pub triplet: bool,
}

impl Default for GridSettings {
    fn default() -> Self {
        Self {
            snap_enabled: true,
            division: 1.0,
            triplet: false,
        }
    }
}

impl GridSettings {
    /// Get the actual grid size in beats.
    pub fn grid_size(&self) -> f64 {
        if self.triplet {
            self.division * 2.0 / 3.0
        } else {
            self.division
        }
    }

    /// Snap a beat position to the grid.
    pub fn snap(&self, position: Beats) -> Beats {
        if !self.snap_enabled {
            return position;
        }
        let grid = self.grid_size();
        Beats((position.0 / grid).round() * grid)
    }
}
