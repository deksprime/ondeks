use crate::transport::Beats;

/// Curve type between automation points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CurveType {
    /// Hold value until next point.
    Hold,
    /// Linear interpolation.
    #[default]
    Linear,
    /// S-curve (smooth).
    SCurve,
    /// Logarithmic (fast start).
    Logarithmic,
    /// Exponential (slow start).
    Exponential,
}

/// A single automation point.
#[derive(Debug, Clone)]
pub struct AutomationPoint {
    /// Position in beats where this point is located.
    pub position: Beats,
    /// Value at this point.
    pub value: f32,
    /// Curve type used for interpolation to the next point.
    pub curve: CurveType,
}

impl AutomationPoint {
    /// Create a new automation point with linear interpolation.
    pub fn new(position: Beats, value: f32) -> Self {
        Self {
            position,
            value,
            curve: CurveType::Linear,
        }
    }

    /// Set the curve type for interpolation to the next point.
    pub fn with_curve(mut self, curve: CurveType) -> Self {
        self.curve = curve;
        self
    }
}

/// Target of automation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AutomationTarget {
    /// Track volume.
    TrackVolume(crate::ids::TrackId),
    /// Track pan.
    TrackPan(crate::ids::TrackId),
    /// Track mute.
    TrackMute(crate::ids::TrackId),
    /// Send level.
    SendLevel {
        /// Source track ID.
        source: crate::ids::TrackId,
        /// Destination track ID.
        destination: crate::ids::TrackId,
    },
    /// Plugin parameter.
    PluginParameter {
        /// Track ID containing the plugin.
        track: crate::ids::TrackId,
        /// Index of the plugin in the track's plugin chain.
        plugin_index: usize,
        /// Parameter ID to automate.
        parameter: crate::ids::ParameterId,
    },
}
