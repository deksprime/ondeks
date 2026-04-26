//! Common traits for UI components.

use ondeks_core::Command;

/// Trait for dispatching commands to the engine.
pub trait CommandDispatcher {
    /// Send a command to the audio engine.
    fn dispatch(&self, command: Command);

    /// Send multiple commands atomically.
    fn dispatch_batch(&self, commands: Vec<Command>) {
        for cmd in commands {
            self.dispatch(cmd);
        }
    }
}

/// Trait for components that can be rendered.
pub trait Renderable {
    /// Returns true if the component needs redrawing.
    fn needs_redraw(&self) -> bool;

    /// Mark the component as needing redraw.
    fn invalidate(&mut self);
}

/// Trait for components that handle keyboard shortcuts.
pub trait KeyboardHandler {
    /// Handle a key press. Returns true if the key was consumed.
    fn handle_key(&mut self, key: KeyEvent) -> bool;
}

/// A keyboard event.
#[derive(Debug, Clone)]
pub struct KeyEvent {
    /// The key that was pressed.
    pub key: Key,
    /// Modifier keys held.
    pub modifiers: Modifiers,
}

/// Keyboard keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    // Letters
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    // Numbers
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
    // Function keys
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    // Navigation
    Left, Right, Up, Down, Home, End, PageUp, PageDown,
    // Editing
    Backspace, Delete, Enter, Tab, Escape, Space,
    // Misc
    Plus, Minus, Comma, Period, Slash,
}

/// Modifier keys.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool, // Cmd on Mac, Win on Windows
}

impl Modifiers {
    pub fn ctrl() -> Self {
        Self { ctrl: true, ..Default::default() }
    }

    pub fn shift() -> Self {
        Self { shift: true, ..Default::default() }
    }

    pub fn alt() -> Self {
        Self { alt: true, ..Default::default() }
    }

    pub fn none() -> Self {
        Self::default()
    }
}
