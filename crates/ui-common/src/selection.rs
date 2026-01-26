//! Selection state management.

use ondeks_core::{TrackId, ClipId, SceneId};
use std::collections::HashSet;
use crate::types::TimeRange;

/// What type of item can be selected.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SelectableItem {
    Track(TrackId),
    Clip(ClipId),
    Scene(SceneId),
    SessionSlot { track: usize, scene: usize },
    AutomationPoint { track: TrackId, lane: usize, point: usize },
    MidiNote { clip: ClipId, note_index: usize },
}

/// Multi-selection state.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    /// Currently selected items
    items: HashSet<SelectableItem>,
    /// Primary/focused item (last clicked)
    primary: Option<SelectableItem>,
    /// Time range selection (for arrangement)
    time_range: Option<TimeRange>,
}

impl Selection {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear all selection.
    pub fn clear(&mut self) {
        self.items.clear();
        self.primary = None;
        self.time_range = None;
    }

    /// Select a single item (clears previous selection).
    pub fn select(&mut self, item: SelectableItem) {
        self.items.clear();
        self.items.insert(item.clone());
        self.primary = Some(item);
    }

    /// Add item to selection (multi-select).
    pub fn add(&mut self, item: SelectableItem) {
        self.items.insert(item.clone());
        self.primary = Some(item);
    }

    /// Toggle item in selection.
    pub fn toggle(&mut self, item: SelectableItem) {
        if self.items.contains(&item) {
            self.items.remove(&item);
            if self.primary.as_ref() == Some(&item) {
                self.primary = self.items.iter().next().cloned();
            }
        } else {
            self.items.insert(item.clone());
            self.primary = Some(item);
        }
    }

    /// Remove item from selection.
    pub fn remove(&mut self, item: &SelectableItem) {
        self.items.remove(item);
        if self.primary.as_ref() == Some(item) {
            self.primary = self.items.iter().next().cloned();
        }
    }

    /// Check if item is selected.
    pub fn is_selected(&self, item: &SelectableItem) -> bool {
        self.items.contains(item)
    }

    /// Check if anything is selected.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get number of selected items.
    pub fn count(&self) -> usize {
        self.items.len()
    }

    /// Get the primary (focused) item.
    pub fn primary(&self) -> Option<&SelectableItem> {
        self.primary.as_ref()
    }

    /// Get all selected items.
    pub fn items(&self) -> impl Iterator<Item = &SelectableItem> {
        self.items.iter()
    }

    /// Get selected tracks.
    pub fn selected_tracks(&self) -> impl Iterator<Item = TrackId> + '_ {
        self.items.iter().filter_map(|item| {
            if let SelectableItem::Track(id) = item {
                Some(*id)
            } else {
                None
            }
        })
    }

    /// Get selected clips.
    pub fn selected_clips(&self) -> impl Iterator<Item = ClipId> + '_ {
        self.items.iter().filter_map(|item| {
            if let SelectableItem::Clip(id) = item {
                Some(*id)
            } else {
                None
            }
        })
    }

    /// Set time range selection.
    pub fn set_time_range(&mut self, range: Option<TimeRange>) {
        self.time_range = range;
    }

    /// Get time range selection.
    pub fn time_range(&self) -> Option<&TimeRange> {
        self.time_range.as_ref()
    }
}

/// Box selection state (for drag-selecting multiple items).
#[derive(Debug, Clone, Default)]
pub struct BoxSelection {
    /// Is box selection currently active?
    pub active: bool,
    /// Starting point of the box
    pub start: crate::types::UiPoint,
    /// Current end point of the box
    pub end: crate::types::UiPoint,
}

impl BoxSelection {
    pub fn start_at(&mut self, point: crate::types::UiPoint) {
        self.active = true;
        self.start = point;
        self.end = point;
    }

    pub fn update(&mut self, point: crate::types::UiPoint) {
        if self.active {
            self.end = point;
        }
    }

    pub fn finish(&mut self) -> Option<crate::types::UiRect> {
        if !self.active {
            return None;
        }
        self.active = false;

        let x = self.start.x.min(self.end.x);
        let y = self.start.y.min(self.end.y);
        let width = (self.end.x - self.start.x).abs();
        let height = (self.end.y - self.start.y).abs();

        Some(crate::types::UiRect::new(x, y, width, height))
    }

    pub fn cancel(&mut self) {
        self.active = false;
    }

    pub fn current_rect(&self) -> Option<crate::types::UiRect> {
        if !self.active {
            return None;
        }

        let x = self.start.x.min(self.end.x);
        let y = self.start.y.min(self.end.y);
        let width = (self.end.x - self.start.x).abs();
        let height = (self.end.y - self.start.y).abs();

        Some(crate::types::UiRect::new(x, y, width, height))
    }
}
