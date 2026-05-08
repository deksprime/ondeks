//! Session view (clip launcher).

mod launcher;
mod view;

pub use launcher::{
    AdvanceResult, ClipLauncher, ClipNote, ClipPlayback, EngineMidiDispatch, LaunchOutcome,
    LaunchQuantize, SlotState, SlotStateChange,
};
pub use view::{ViewManager, ViewMode};
