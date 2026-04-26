//! User preferences.

use crate::theme::Theme;
use crate::types::GridSettings;

/// User interface preferences.
#[derive(Debug, Clone)]
pub struct Preferences {
    /// Selected theme
    pub theme: Theme,
    /// Default grid settings
    pub grid: GridSettings,
    /// Auto-save interval in seconds (0 = disabled)
    pub auto_save_interval: u32,
    /// Show tooltips
    pub show_tooltips: bool,
    /// Animation enabled
    pub animations_enabled: bool,
    /// Meter refresh rate (fps)
    pub meter_fps: u32,
    /// Follow playhead in arrangement
    pub follow_playhead: bool,
    /// Default zoom level
    pub default_zoom: f32,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: Theme::dark(),
            grid: GridSettings::default(),
            auto_save_interval: 60,
            show_tooltips: true,
            animations_enabled: true,
            meter_fps: 30,
            follow_playhead: true,
            default_zoom: 50.0,
        }
    }
}

impl Preferences {
    pub fn new() -> Self {
        Self::default()
    }
}
