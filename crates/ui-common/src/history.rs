//! Undo/redo history management.

use crate::commands::UiCommand;

/// A record of an action that can be undone.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// Description of the action
    pub description: String,
    /// Command to undo (reverse action)
    pub undo: UiCommand,
    /// Command to redo (repeat action)
    pub redo: UiCommand,
}

/// Undo/redo history manager.
#[derive(Debug, Default)]
pub struct History {
    /// Stack of actions that can be undone
    undo_stack: Vec<HistoryEntry>,
    /// Stack of actions that can be redone
    redo_stack: Vec<HistoryEntry>,
    /// Maximum history size
    max_size: usize,
}

impl History {
    pub fn new(max_size: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_size,
        }
    }

    /// Record an action.
    pub fn record(&mut self, entry: HistoryEntry) {
        // Clear redo stack when new action is performed
        self.redo_stack.clear();

        self.undo_stack.push(entry);

        // Trim to max size
        while self.undo_stack.len() > self.max_size {
            self.undo_stack.remove(0);
        }
    }

    /// Get the next action to undo.
    pub fn undo(&mut self) -> Option<UiCommand> {
        if let Some(entry) = self.undo_stack.pop() {
            let cmd = entry.undo.clone();
            self.redo_stack.push(entry);
            Some(cmd)
        } else {
            None
        }
    }

    /// Get the next action to redo.
    pub fn redo(&mut self) -> Option<UiCommand> {
        if let Some(entry) = self.redo_stack.pop() {
            let cmd = entry.redo.clone();
            self.undo_stack.push(entry);
            Some(cmd)
        } else {
            None
        }
    }

    /// Check if undo is available.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Check if redo is available.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Get description of next undo action.
    pub fn undo_description(&self) -> Option<&str> {
        self.undo_stack.last().map(|e| e.description.as_str())
    }

    /// Get description of next redo action.
    pub fn redo_description(&self) -> Option<&str> {
        self.redo_stack.last().map(|e| e.description.as_str())
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}
