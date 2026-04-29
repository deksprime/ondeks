//! Piano roll editor view.
//!
//! A clip is a horizontal grid: time on X (in beats), pitch on Y (MIDI 0-127,
//! displayed as a labelled keyboard column on the left). Notes are rendered
//! as colored rectangles. Interaction model for Slice 8 MVP:
//!
//! - **Click empty grid** → emit `AddMidiNote` at the snapped time / pitch
//!   under the cursor, length = current snap (1/8 beat).
//! - **Click a note** → select it. Click another note → switches selection.
//!   Click outside any note → deselect.
//! - **Drag selected note** (body) → emit `MoveMidiNote` to the snapped new
//!   time + new pitch under the cursor each drag tick. Dispatched on commit
//!   (drag stop) so undo is one entry per gesture.
//! - **Drag right edge of a note** → emit `ResizeMidiNote`.
//! - **Delete / Backspace** → `RemoveMidiNote` on the selected note.
//! - **Esc** → deselect.
//!
//! Snap is fixed at 1/8 beat for the MVP; a snap picker is a polish slice.
//! Multi-select, velocity lane, and other tools (erase / split) come later.

use eframe::egui;
use ondeks_core::midi::{Note, Velocity};
use ondeks_core::project::MidiNote;
use ondeks_core::transport::Beats;

/// Piano roll widget. Owns nothing — driven by a borrowed clip ref and a
/// borrowed selection state. Returns the actions the user took this frame.
pub struct PianoRollView<'a> {
    /// The notes the clip currently contains, in stable index order.
    notes: &'a [MidiNote],
    /// Total clip length in beats (defines the visible time range).
    clip_length: Beats,
    /// Currently-selected note index (if any). Drives highlight and Delete.
    selected: Option<usize>,
    /// In-progress drag state owned by the caller and threaded through; lets
    /// the widget render the ghost note while dragging.
    drag: &'a mut DragState,
}

/// Per-app state for an in-flight drag (move or resize).
#[derive(Debug, Clone, Default)]
pub struct DragState {
    /// What's being dragged.
    pub mode: DragMode,
    /// Note index being dragged (move/resize).
    pub note_index: Option<usize>,
    /// Pointer beat at drag start (unsnapped, raw). Used as the anchor for
    /// delta-based movement: ghost / commit positions are derived from
    /// `pointer_beat - start_beat` snapped to the grid.
    pub start_beat: f64,
    /// Pointer pitch at drag start (raw). Pitch deltas are integer
    /// semitones — no snap needed.
    pub start_pitch: u8,
    /// Note's pre-drag start time and pitch — combined with deltas to
    /// derive the proposed ghost position and the commit value on release.
    pub original_time: Beats,
    pub original_pitch: u8,
    pub original_length: Beats,
    /// Last MIDI pitch we fired a preview NoteOn for during this gesture.
    /// Used to scrub-fire each new pitch the cursor crosses (drag-to-scrub
    /// during a Move; click-and-drag down the keyboard column).
    pub last_previewed_pitch: Option<u8>,
}

/// What kind of drag is in progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DragMode {
    /// No drag.
    #[default]
    None,
    /// Moving a note's start position (time + pitch).
    Move,
    /// Resizing a note's length.
    Resize,
}

/// Result of one frame of interactions.
#[derive(Debug, Default)]
pub struct PianoRollResponse {
    /// Add a note at this (time, pitch). Length defaults to 1/8 (the snap).
    pub add_note: Option<(Beats, Note)>,
    /// Select this note index (or `None` to clear selection).
    pub set_selected: Option<Option<usize>>,
    /// Commit a move: (note_index, new_time, new_pitch).
    pub commit_move: Option<(usize, Beats, Note)>,
    /// Commit a resize: (note_index, new_length).
    pub commit_resize: Option<(usize, Beats)>,
    /// Delete the currently-selected note (caller looks up its index).
    pub delete_selected: bool,
    /// Switch back to the session view.
    pub close_clicked: bool,
    /// Briefly fire this pitch on the track's instrument. Set by clicking a
    /// piano key on the left strip, or by crossing into a new pitch row
    /// while dragging a note vertically. The caller uses its existing
    /// preview-note machinery (NoteOn now, NoteOff queued ~250 ms later).
    pub preview_pitch: Option<Note>,
}

impl<'a> PianoRollView<'a> {
    /// Create a new piano roll for the given clip.
    pub fn new(
        notes: &'a [MidiNote],
        clip_length: Beats,
        selected: Option<usize>,
        drag: &'a mut DragState,
    ) -> Self {
        Self { notes, clip_length, selected, drag }
    }

    /// Render and collect responses for this frame.
    pub fn show(self, ui: &mut egui::Ui) -> PianoRollResponse {
        let mut response = PianoRollResponse::default();

        // Top toolbar: title + close button.
        ui.horizontal(|ui| {
            ui.heading("Piano Roll");
            ui.label(
                egui::RichText::new(format!("({} beats)", self.clip_length.0 as u32))
                    .small()
                    .color(egui::Color32::from_gray(140)),
            );
            if ui.button("← Session").clicked() {
                response.close_clicked = true;
            }
            ui.label(
                egui::RichText::new("Click grid to add  •  Click note to select  •  Drag to move  •  Drag right edge to resize  •  Del to remove")
                    .small()
                    .color(egui::Color32::from_gray(120)),
            );
        });

        ui.separator();

        // Layout constants.
        let key_label_width = 44.0_f32;
        let time_ruler_height = 24.0_f32;
        let row_height = 14.0_f32; // pixels per semitone
        let lowest_pitch: u8 = 36; // C2
        let highest_pitch: u8 = 96; // C7
        let row_count = (highest_pitch - lowest_pitch + 1) as f32;

        let length_beats = self.clip_length.0.max(1.0) as f32;
        let beat_width = 60.0_f32; // pixels per beat
        let snap = Beats(0.125); // 1/8 beat

        let total_width = key_label_width + length_beats * beat_width;
        let total_height = time_ruler_height + row_count * row_height;

        // Allocate the full canvas; the user can scroll if too big. Disable
        // drag-to-scroll so a primary-button drag inside the grid doesn't get
        // intercepted by the scroll area — the piano roll claims drags for
        // moving / resizing notes (especially vertical, which would otherwise
        // pan the view when the content is taller than the viewport).
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .drag_to_scroll(false)
            .show(ui, |ui| {
                let (rect, _resp_outer) = ui.allocate_exact_size(
                    egui::vec2(total_width, total_height),
                    egui::Sense::hover(),
                );
                let painter = ui.painter_at(rect);

                // -------- Time ruler (top) --------
                let ruler_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.left() + key_label_width, rect.top()),
                    egui::vec2(length_beats * beat_width, time_ruler_height),
                );
                painter.rect_filled(ruler_rect, 0.0, egui::Color32::from_gray(40));
                for beat in 0..=(length_beats as i32) {
                    let x = ruler_rect.left() + beat as f32 * beat_width;
                    painter.line_segment(
                        [egui::pos2(x, ruler_rect.top()), egui::pos2(x, ruler_rect.bottom())],
                        egui::Stroke::new(1.0, egui::Color32::from_gray(70)),
                    );
                    painter.text(
                        egui::pos2(x + 3.0, ruler_rect.top() + 2.0),
                        egui::Align2::LEFT_TOP,
                        format!("{}", beat + 1),
                        egui::FontId::monospace(10.0),
                        egui::Color32::from_gray(180),
                    );
                }

                // -------- Piano keys + grid rows --------
                for i in 0..(row_count as u8) {
                    let pitch = highest_pitch - i; // top row = highest pitch
                    let y = rect.top() + time_ruler_height + i as f32 * row_height;
                    let row_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.left(), y),
                        egui::vec2(total_width, row_height),
                    );

                    let is_black = matches!(pitch % 12, 1 | 3 | 6 | 8 | 10);
                    let is_c = pitch % 12 == 0;

                    // Row background — alternating tint, darker for black-key rows.
                    let row_bg = if is_black {
                        egui::Color32::from_gray(28)
                    } else {
                        egui::Color32::from_gray(34)
                    };
                    painter.rect_filled(row_rect, 0.0, row_bg);

                    // Octave divider line under each C (full width — also
                    // separates the white C from the white B below it).
                    if is_c {
                        painter.line_segment(
                            [
                                egui::pos2(rect.left(), y + row_height),
                                egui::pos2(rect.right(), y + row_height),
                            ],
                            egui::Stroke::new(1.0, egui::Color32::from_gray(70)),
                        );
                    }

                    // Key label column on the left.
                    let label_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.left(), y),
                        egui::vec2(key_label_width, row_height),
                    );
                    let label_bg = if is_black {
                        egui::Color32::from_gray(20)
                    } else {
                        egui::Color32::from_gray(220)
                    };
                    painter.rect_filled(label_rect, 0.0, label_bg);

                    // White-key separator: F sits directly above E (no black
                    // key between), and C sits directly above B (octave
                    // wrap, already handled above by the octave divider).
                    // Without this line F and E (and originally B and C)
                    // visually merge into one fat key.
                    if !is_black && pitch % 12 == 5 {
                        painter.line_segment(
                            [
                                egui::pos2(rect.left(), y + row_height),
                                egui::pos2(rect.left() + key_label_width, y + row_height),
                            ],
                            egui::Stroke::new(1.0, egui::Color32::from_gray(120)),
                        );
                    }
                    if is_c {
                        let octave = (pitch as i32 / 12) - 1;
                        painter.text(
                            label_rect.right_top() + egui::vec2(-3.0, 1.0),
                            egui::Align2::RIGHT_TOP,
                            format!("C{}", octave),
                            egui::FontId::monospace(9.0),
                            egui::Color32::from_gray(60),
                        );
                    }
                }

                // -------- Vertical sub-beat grid (1/8 ticks) --------
                let ticks_per_beat = (1.0 / snap.0).round() as i32;
                for tick in 0..=(length_beats as i32 * ticks_per_beat) {
                    let x = rect.left() + key_label_width + tick as f32 * beat_width / ticks_per_beat as f32;
                    let on_beat = tick % ticks_per_beat == 0;
                    let stroke = if on_beat {
                        egui::Stroke::new(1.0, egui::Color32::from_gray(70))
                    } else {
                        egui::Stroke::new(1.0, egui::Color32::from_gray(45))
                    };
                    painter.line_segment(
                        [
                            egui::pos2(x, rect.top() + time_ruler_height),
                            egui::pos2(x, rect.bottom()),
                        ],
                        stroke,
                    );
                }

                // -------- Note rectangles --------
                let grid_origin =
                    egui::pos2(rect.left() + key_label_width, rect.top() + time_ruler_height);

                let note_rect_for = |n: &MidiNote| -> Option<egui::Rect> {
                    if n.pitch.raw() < lowest_pitch || n.pitch.raw() > highest_pitch {
                        return None;
                    }
                    let row = (highest_pitch - n.pitch.raw()) as f32;
                    let x = grid_origin.x + n.time.0 as f32 * beat_width;
                    let y = grid_origin.y + row * row_height;
                    let w = (n.length.0 as f32 * beat_width).max(2.0);
                    Some(egui::Rect::from_min_size(
                        egui::pos2(x, y),
                        egui::vec2(w, row_height - 1.0),
                    ))
                };

                // -------- Pointer interaction --------
                let pointer = ui.ctx().pointer_hover_pos();
                let pointer_in_grid = pointer
                    .map(|p| {
                        p.x >= grid_origin.x
                            && p.x <= rect.right()
                            && p.y >= grid_origin.y
                            && p.y <= rect.bottom()
                    })
                    .unwrap_or(false);

                let primary_down = ui.ctx().input(|i| i.pointer.primary_down());
                let primary_pressed = ui.ctx().input(|i| i.pointer.primary_pressed());
                let primary_released = ui.ctx().input(|i| i.pointer.primary_released());

                // Translate pointer → (beat, pitch) in grid coords.
                let pointer_to_cell = |p: egui::Pos2| -> (f64, u8) {
                    let beat = ((p.x - grid_origin.x) / beat_width).max(0.0) as f64;
                    let row = ((p.y - grid_origin.y) / row_height).floor() as i32;
                    let pitch = (highest_pitch as i32 - row).clamp(0, 127) as u8;
                    (beat, pitch)
                };
                let snap_beat = |b: f64| (b / snap.0).round() * snap.0;

                // Hit-test against existing notes.
                let mut hit_note_idx: Option<usize> = None;
                let mut hit_resize_handle = false;
                if let Some(p) = pointer {
                    if pointer_in_grid {
                        for (idx, n) in self.notes.iter().enumerate().rev() {
                            let Some(nrect) = note_rect_for(n) else { continue };
                            if nrect.contains(p) {
                                hit_note_idx = Some(idx);
                                let resize_zone = egui::Rect::from_min_max(
                                    egui::pos2(nrect.right() - 5.0, nrect.top()),
                                    nrect.right_bottom(),
                                );
                                hit_resize_handle = resize_zone.contains(p);
                                break;
                            }
                        }
                    }
                }

                // Whether the pointer is over the left keyboard strip
                // (clickable piano keys for pitch preview).
                let pointer_in_keyboard = pointer
                    .map(|p| {
                        p.x >= rect.left()
                            && p.x < rect.left() + key_label_width
                            && p.y >= rect.top() + time_ruler_height
                            && p.y <= rect.bottom()
                    })
                    .unwrap_or(false);

                // Cursor hint: show the double-arrow over a resize handle, an
                // open hand over a movable note body, the pointing-hand over
                // the clickable keyboard strip, and a closed hand while
                // actively dragging. egui restores the default after the
                // frame so we just set it whenever appropriate.
                let cursor_icon = match self.drag.mode {
                    DragMode::Resize => Some(egui::CursorIcon::ResizeHorizontal),
                    DragMode::Move => Some(egui::CursorIcon::Grabbing),
                    DragMode::None => {
                        if hit_resize_handle {
                            Some(egui::CursorIcon::ResizeHorizontal)
                        } else if hit_note_idx.is_some() {
                            Some(egui::CursorIcon::Grab)
                        } else if pointer_in_keyboard {
                            Some(egui::CursorIcon::PointingHand)
                        } else {
                            None
                        }
                    }
                };
                if let Some(c) = cursor_icon {
                    ui.ctx().set_cursor_icon(c);
                }

                // Press on the keyboard column → fire a preview for that
                // pitch. The held-and-dragged glissando case is handled
                // below via the unified scrub block.
                if primary_pressed && pointer_in_keyboard {
                    if let Some(p) = pointer {
                        let (_, pitch_raw) = pointer_to_cell(p);
                        if let Ok(pitch) = Note::new(pitch_raw) {
                            response.preview_pitch = Some(pitch);
                            self.drag.last_previewed_pitch = Some(pitch_raw);
                        }
                    }
                }

                // Press → start drag, select, or add note.
                if primary_pressed && pointer_in_grid {
                    if let Some(p) = pointer {
                        if let Some(idx) = hit_note_idx {
                            // Begin drag (resize if on right edge, else move).
                            // Anchor by *click position*, not the note's
                            // start: deltas are computed from where the user
                            // clicked, so a long note doesn't jump to align
                            // its start with the cursor.
                            let n = &self.notes[idx];
                            let (click_beat_raw, click_pitch_raw) = pointer_to_cell(p);
                            self.drag.mode = if hit_resize_handle {
                                DragMode::Resize
                            } else {
                                DragMode::Move
                            };
                            self.drag.note_index = Some(idx);
                            self.drag.start_beat = click_beat_raw;
                            self.drag.start_pitch = click_pitch_raw;
                            self.drag.original_time = n.time;
                            self.drag.original_pitch = n.pitch.raw();
                            self.drag.original_length = n.length;
                            response.set_selected = Some(Some(idx));
                        } else {
                            // Empty cell → emit AddMidiNote at snapped pos.
                            let (beat_raw, pitch) = pointer_to_cell(p);
                            let snapped = snap_beat(beat_raw);
                            if let Ok(n) = Note::new(pitch) {
                                response.add_note = Some((Beats(snapped), n));
                            }
                            response.set_selected = Some(None);
                        }
                    }
                }

                // -------- Compute proposed drag deltas --------
                // Whether the cursor has moved enough since press to count as
                // a "real" drag — until we cross a snap unit (or a semitone),
                // the user is effectively just holding still and we want to
                // show the original note unchanged, no ghost.
                let drag_proposal = if self.drag.mode != DragMode::None {
                    if let (Some(idx), Some(p)) = (self.drag.note_index, pointer) {
                        let (cur_beat, cur_pitch) = pointer_to_cell(p);
                        let raw_delta_beat = cur_beat - self.drag.start_beat;
                        let snapped_delta_beat = snap_beat(raw_delta_beat);
                        let delta_pitch =
                            cur_pitch as i32 - self.drag.start_pitch as i32;
                        let original = &self.notes[idx];
                        match self.drag.mode {
                            DragMode::Move => {
                                if snapped_delta_beat != 0.0 || delta_pitch != 0 {
                                    let new_time =
                                        (original.time.0 + snapped_delta_beat).max(0.0);
                                    let new_pitch_raw = (original.pitch.raw() as i32
                                        + delta_pitch)
                                        .clamp(0, 127)
                                        as u8;
                                    Some((idx, new_time, new_pitch_raw, original.length.0))
                                } else {
                                    None
                                }
                            }
                            DragMode::Resize => {
                                if snapped_delta_beat != 0.0 {
                                    let new_length =
                                        (original.length.0 + snapped_delta_beat).max(snap.0);
                                    Some((idx, original.time.0, original.pitch.raw(), new_length))
                                } else {
                                    None
                                }
                            }
                            DragMode::None => None,
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                // -------- Note rectangles --------
                // (Render after we know `drag_proposal` so we can dim the
                // dragged note only when there's actually a ghost to show.)
                for (idx, n) in self.notes.iter().enumerate() {
                    let Some(nrect) = note_rect_for(n) else { continue };
                    let is_selected = self.selected == Some(idx);
                    // The dragged note dims only once the user has moved far
                    // enough to register a delta — clicking-and-holding
                    // keeps the note solid.
                    let is_dragged = drag_proposal
                        .map(|(i, _, _, _)| i == idx)
                        .unwrap_or(false);
                    let fill = if is_dragged {
                        egui::Color32::from_rgba_unmultiplied(80, 200, 255, 70)
                    } else if is_selected {
                        egui::Color32::from_rgb(255, 180, 80)
                    } else {
                        egui::Color32::from_rgb(80, 200, 255)
                    };
                    painter.rect_filled(nrect, 2.0, fill);
                    let stroke_color = if is_dragged {
                        egui::Color32::from_rgba_unmultiplied(0, 0, 0, 80)
                    } else {
                        egui::Color32::BLACK
                    };
                    painter.rect_stroke(nrect, 2.0, egui::Stroke::new(1.0, stroke_color));
                }

                // -------- Ghost rendering --------
                // Only renders once the drag proposal is non-trivial.
                if let Some((idx, ghost_time, ghost_pitch_raw, ghost_length)) = drag_proposal {
                    let original = &self.notes[idx];
                    if let Ok(ghost_pitch) = Note::new(ghost_pitch_raw) {
                        let ghost = MidiNote {
                            time: Beats(ghost_time),
                            length: Beats(ghost_length),
                            pitch: ghost_pitch,
                            velocity: original.velocity,
                            channel: original.channel,
                        };
                        if let Some(grect) = note_rect_for(&ghost) {
                            painter.rect_filled(
                                grect,
                                2.0,
                                egui::Color32::from_rgba_unmultiplied(255, 200, 100, 130),
                            );
                            painter.rect_stroke(
                                grect,
                                2.0,
                                egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 200, 100)),
                            );
                        }
                    }
                }
                let _ = primary_down;

                // -------- Pitch scrubbing during drag / keyboard slide --------
                // Fire a preview NoteOn whenever the *target pitch* changes
                // during a held gesture: dragging a note vertically across
                // pitches (item from the slice plan), or sliding down the
                // keyboard column with the mouse held. One-shot per pitch
                // crossing; the app queues a NoteOff ~250 ms later.
                if primary_down {
                    let scrub_pitch_raw: Option<u8> = if pointer_in_keyboard {
                        pointer.map(|p| pointer_to_cell(p).1)
                    } else if self.drag.mode == DragMode::Move {
                        drag_proposal.map(|(_, _, p, _)| p)
                    } else {
                        None
                    };
                    if let Some(p) = scrub_pitch_raw {
                        if Some(p) != self.drag.last_previewed_pitch {
                            if let Ok(pitch) = Note::new(p) {
                                response.preview_pitch = Some(pitch);
                                self.drag.last_previewed_pitch = Some(p);
                            }
                        }
                    }
                } else {
                    // Mouse not held → reset so the next press fires fresh.
                    self.drag.last_previewed_pitch = None;
                }

                // Release → commit drag (only if there was a non-trivial
                // proposal; clicking and releasing without moving is a
                // selection-only gesture, no commit).
                if primary_released && self.drag.mode != DragMode::None {
                    if let Some((idx, new_time, new_pitch_raw, new_length)) = drag_proposal {
                        match self.drag.mode {
                            DragMode::Move => {
                                if let Ok(new_pitch) = Note::new(new_pitch_raw) {
                                    response.commit_move =
                                        Some((idx, Beats(new_time), new_pitch));
                                }
                            }
                            DragMode::Resize => {
                                response.commit_resize = Some((idx, Beats(new_length)));
                            }
                            DragMode::None => {}
                        }
                    }
                    *self.drag = DragState::default();
                }

                // Keyboard: Delete/Backspace removes selected; Escape deselects.
                let (delete, escape) = ui.ctx().input(|i| {
                    (
                        i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
                        i.key_pressed(egui::Key::Escape),
                    )
                });
                if delete && self.selected.is_some() {
                    response.delete_selected = true;
                }
                if escape && self.selected.is_some() {
                    response.set_selected = Some(None);
                }
            });

        // Keep clippy quiet for the unused velocity import — used by callers.
        let _ = Velocity::MAX;

        response
    }
}
