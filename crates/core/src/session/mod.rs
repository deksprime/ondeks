//! Session view (clip launcher).

mod launcher;
mod view;

pub use launcher::{ClipLauncher, SlotState, LaunchQuantize, LaunchEvent};
pub use view::{ViewManager, ViewMode};
