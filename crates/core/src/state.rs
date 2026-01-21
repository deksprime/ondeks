//! Engine state management.

/// The current state of the audio engine.
#[derive(Debug, Clone, Default)]
pub struct State {
    // State will be expanded in later phases
}

impl State {
    /// Create a new empty state.
    pub fn new() -> Self {
        Self::default()
    }
}
