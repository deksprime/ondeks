//! On-screen piano keyboard widget.
//!
//! Two octaves (C3..B4). Mouse click/drag + QWERTY keyboard (Ableton-style
//! A W S E D F T G Y H U J mapping for the lower octave) both drive MIDI
//! output. Returns a list of `MidiEvent`s generated this frame, derived by
//! diffing the set of held notes against last frame's set.

use std::collections::BTreeSet;

use eframe::egui::{self, Color32, Key, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use ondeks_core::midi::{Channel, MidiEvent, Note, Velocity};

/// MIDI note at the leftmost key (C3).
const START_NOTE: u8 = 48;
/// Keys rendered: two full octaves, C3..B4.
const KEY_COUNT: u8 = 24;
/// Fixed click velocity. Y-position velocity is a polish slice (Slice 28+).
const CLICK_VELOCITY: u8 = 100;

fn is_black(semitone_offset: u8) -> bool {
    matches!(semitone_offset % 12, 1 | 3 | 6 | 8 | 10)
}

/// Map QWERTY keys to MIDI note offsets from [`START_NOTE`].
/// Ableton/Live convention: lower octave starts on A=C.
fn qwerty_to_offset(key: Key) -> Option<u8> {
    Some(match key {
        Key::A => 0,   // C3
        Key::W => 1,   // C#3
        Key::S => 2,   // D3
        Key::E => 3,   // D#3
        Key::D => 4,   // E3
        Key::F => 5,   // F3
        Key::T => 6,   // F#3
        Key::G => 7,   // G3
        Key::Y => 8,   // G#3
        Key::H => 9,   // A3
        Key::U => 10,  // A#3
        Key::J => 11,  // B3
        Key::K => 12,  // C4
        Key::O => 13,  // C#4
        Key::L => 14,  // D4
        _ => return None,
    })
}

/// Result of one frame of keyboard rendering.
#[derive(Debug, Default)]
pub struct PianoKeyboardResponse {
    /// MIDI events generated this frame, in time order (NoteOffs before NoteOns).
    pub events: Vec<MidiEvent>,
}

/// On-screen piano keyboard widget.
pub struct PianoKeyboard {
    height: f32,
}

impl Default for PianoKeyboard {
    fn default() -> Self {
        Self { height: 90.0 }
    }
}

impl PianoKeyboard {
    /// Create a default keyboard (~90 px tall).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the widget height in logical pixels.
    pub fn height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }

    /// Render the keyboard and return generated MIDI events for this frame.
    pub fn show(self, ui: &mut Ui) -> PianoKeyboardResponse {
        let desired = Vec2::new(ui.available_width().max(400.0), self.height);
        let (rect, response) = ui.allocate_exact_size(desired, Sense::click_and_drag());
        let painter = ui.painter_at(rect);

        // Layout: 14 white keys across the full width; 10 black keys overlaid.
        let white_count = (0..KEY_COUNT).filter(|i| !is_black(*i)).count() as f32;
        let white_w = rect.width() / white_count;
        let black_w = white_w * 0.6;
        let black_h = rect.height() * 0.6;

        // Build per-key rect list: (note, rect, is_black).
        let mut key_rects: Vec<(u8, Rect, bool)> = Vec::with_capacity(KEY_COUNT as usize);
        let mut white_idx = 0.0f32;
        for i in 0..KEY_COUNT {
            let note = START_NOTE + i;
            if is_black(i) {
                let cx = rect.left() + white_idx * white_w;
                let r = Rect::from_min_size(
                    Pos2::new(cx - black_w / 2.0, rect.top()),
                    Vec2::new(black_w, black_h),
                );
                key_rects.push((note, r, true));
            } else {
                let x = rect.left() + white_idx * white_w;
                let r = Rect::from_min_size(
                    Pos2::new(x, rect.top()),
                    Vec2::new(white_w, rect.height()),
                );
                key_rects.push((note, r, false));
                white_idx += 1.0;
            }
        }

        // Derive currently-held notes from input.
        let mut current: BTreeSet<u8> = BTreeSet::new();

        // Mouse hit-test (black keys first — they're rendered on top).
        let mouse_down = response.is_pointer_button_down_on();
        if mouse_down {
            if let Some(p) = response.interact_pointer_pos() {
                if rect.contains(p) {
                    let mut hit: Option<u8> = None;
                    for &(note, r, is_bk) in &key_rects {
                        if is_bk && r.contains(p) {
                            hit = Some(note);
                            break;
                        }
                    }
                    if hit.is_none() {
                        for &(note, r, is_bk) in &key_rects {
                            if !is_bk && r.contains(p) {
                                hit = Some(note);
                                break;
                            }
                        }
                    }
                    if let Some(n) = hit {
                        current.insert(n);
                    }
                }
            }
        }

        // QWERTY input (held keys). Only read input when the widget has focus
        // or no text field has focus — egui filters text input automatically
        // when a TextEdit is active, so we can just query key_down globally.
        let inputs = ui.ctx().input(|i| {
            let mut held: Vec<u8> = Vec::new();
            for &key in &[
                Key::A, Key::W, Key::S, Key::E, Key::D, Key::F, Key::T,
                Key::G, Key::Y, Key::H, Key::U, Key::J, Key::K, Key::O, Key::L,
            ] {
                if i.key_down(key) {
                    if let Some(off) = qwerty_to_offset(key) {
                        held.push(START_NOTE + off);
                    }
                }
            }
            held
        });
        for n in inputs {
            current.insert(n);
        }

        // Diff against previous frame's held set to produce events.
        let memory_id = response.id.with("held_notes");
        let previous: BTreeSet<u8> =
            ui.memory_mut(|m| m.data.get_temp(memory_id)).unwrap_or_default();

        let mut events = PianoKeyboardResponse::default();
        let ch = Channel::new(0).expect("channel 0 is always valid");
        // NoteOff for notes that were held but no longer are.
        for &n in previous.difference(&current) {
            if let Ok(note) = Note::new(n) {
                events.events.push(MidiEvent::note_off(ch, note));
            }
        }
        // NoteOn for newly-held notes.
        for &n in current.difference(&previous) {
            if let Ok(note) = Note::new(n) {
                let velocity = Velocity::new(CLICK_VELOCITY).expect("100 is a valid velocity");
                events.events.push(MidiEvent::NoteOn {
                    channel: ch,
                    note,
                    velocity,
                });
            }
        }

        ui.memory_mut(|m| m.data.insert_temp(memory_id, current.clone()));

        // Paint. White keys first, then black keys on top.
        for &(note, r, is_bk) in &key_rects {
            if is_bk {
                continue;
            }
            let held = current.contains(&note);
            let fill = if held {
                Color32::from_rgb(180, 220, 255)
            } else {
                Color32::from_gray(235)
            };
            painter.rect_filled(r, 2.0, fill);
            painter.rect_stroke(r, 2.0, Stroke::new(1.0, Color32::from_gray(90)));
            // Octave marker under C notes.
            if note % 12 == 0 {
                let octave = (note / 12) as i32 - 1;
                painter.text(
                    Pos2::new(r.left() + 3.0, r.bottom() - 14.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("C{}", octave),
                    egui::FontId::monospace(10.0),
                    Color32::from_gray(110),
                );
            }
        }
        for &(note, r, is_bk) in &key_rects {
            if !is_bk {
                continue;
            }
            let held = current.contains(&note);
            let fill = if held {
                Color32::from_rgb(80, 140, 220)
            } else {
                Color32::from_gray(25)
            };
            painter.rect_filled(r, 2.0, fill);
            painter.rect_stroke(r, 2.0, Stroke::new(1.0, Color32::from_gray(60)));
        }

        events
    }
}
