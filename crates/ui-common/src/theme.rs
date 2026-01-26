//! Theme and color definitions.

/// RGBA color (0.0 to 1.0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl ThemeColor {
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}

/// Complete UI theme.
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// Ableton-inspired dark theme.
    pub fn dark() -> Self {
        Self {
            name: "Dark".to_string(),
        }
    }

    /// Light theme.
    pub fn light() -> Self {
        Self {
            name: "Light".to_string(),
        }
    }
}
