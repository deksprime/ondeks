//! Keyboard shortcut definitions and mapping.

use crate::commands::UiCommand;
use crate::traits::{Key, Modifiers, KeyEvent};
use std::collections::HashMap;

/// A keyboard shortcut binding.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub key: Key,
    pub modifiers: Modifiers,
}

impl Shortcut {
    pub fn new(key: Key, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }

    pub fn key(key: Key) -> Self {
        Self::new(key, Modifiers::none())
    }

    pub fn ctrl(key: Key) -> Self {
        Self::new(key, Modifiers::ctrl())
    }
}

/// Action triggered by a shortcut.
#[derive(Debug, Clone)]
pub enum ShortcutAction {
    Command(UiCommand),
    Named(String),
}

/// Keyboard shortcut map.
#[derive(Debug, Clone)]
pub struct ShortcutMap {
    bindings: HashMap<Shortcut, ShortcutAction>,
}

impl Default for ShortcutMap {
    fn default() -> Self {
        Self::new()
    }
}

impl ShortcutMap {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    pub fn bind(&mut self, shortcut: Shortcut, action: ShortcutAction) {
        self.bindings.insert(shortcut, action);
    }

    pub fn get_action(&self, event: &KeyEvent) -> Option<&ShortcutAction> {
        for (shortcut, action) in &self.bindings {
            if shortcut.key == event.key
                && shortcut.modifiers.ctrl == event.modifiers.ctrl
                && shortcut.modifiers.shift == event.modifiers.shift
                && shortcut.modifiers.alt == event.modifiers.alt
                && shortcut.modifiers.meta == event.modifiers.meta
            {
                return Some(action);
            }
        }
        None
    }
}
