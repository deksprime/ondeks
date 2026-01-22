use super::launcher::ClipLauncher;
use crate::arrangement::ArrangementPlayback;

/// Which view/mode is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Session,
    Arrangement,
}

/// Manages the current view mode.
#[derive(Debug)]
pub struct ViewManager {
    pub mode: ViewMode,
    pub session: ClipLauncher,
    pub arrangement: ArrangementPlayback,
}

impl ViewManager {
    pub fn new() -> Self {
        Self {
            mode: ViewMode::Session,
            session: ClipLauncher::new(),
            arrangement: ArrangementPlayback::new(),
        }
    }

    /// Switch to session view.
    pub fn switch_to_session(&mut self) {
        self.mode = ViewMode::Session;
        self.arrangement.stop();
    }

    /// Switch to arrangement view.
    pub fn switch_to_arrangement(&mut self) {
        self.mode = ViewMode::Arrangement;
        self.session.stop_all();
    }
}

impl Default for ViewManager {
    fn default() -> Self {
        Self::new()
    }
}
