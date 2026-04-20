//! Session view (clip launcher grid).

use eframe::egui;
use ondeks_ui_common::{SessionViewModel, SlotViewModel, SlotColorHint, Selection, SelectableItem};
use ondeks_core::session::SlotState;
use ondeks_core::{Color, TrackId};
use ondeks_core::project::TrackType;

/// Action picked from a track header context menu.
#[derive(Debug, Clone)]
pub enum TrackMenuAction {
    /// Begin renaming this track. UI should switch the header to a TextEdit.
    StartRename,
    /// Duplicate this track.
    Duplicate,
    /// Delete this track.
    Delete,
    /// Move one position toward the top of the list.
    MoveUp,
    /// Move one position toward the bottom.
    MoveDown,
    /// Set the track's display color.
    SetColor(Color),
}

/// Session view widget.
pub struct SessionView<'a> {
    vm: &'a SessionViewModel,
    selection: &'a Selection,
    /// If set, the track with this id is being renamed and the buffer holds
    /// the in-progress name. The widget shows a TextEdit in place of the
    /// label until the user commits (Enter or focus-loss) or cancels (Esc).
    rename: Option<(TrackId, &'a mut String)>,
}

impl<'a> SessionView<'a> {
    /// Create a new session view.
    pub fn new(vm: &'a SessionViewModel, selection: &'a Selection) -> Self {
        Self { vm, selection, rename: None }
    }

    /// Activate inline rename on a specific track with the provided buffer.
    pub fn with_rename(mut self, target: TrackId, buffer: &'a mut String) -> Self {
        self.rename = Some((target, buffer));
        self
    }
}

/// Response from session view.
#[derive(Debug, Default)]
pub struct SessionViewResponse {
    /// Slot that was clicked (track, scene)
    pub slot_clicked: Option<(usize, usize)>,
    /// Scene launch button clicked
    pub scene_launched: Option<usize>,
    /// Track stop button clicked
    pub track_stopped: Option<usize>,
    /// Stop all button clicked
    pub stop_all_clicked: bool,
    /// Slot selection changed
    pub selection_changed: bool,
    /// User picked an action from a track header context menu.
    pub track_menu_action: Option<(TrackId, TrackMenuAction)>,
    /// User clicked the "+" add-track button with this track type.
    pub add_track_clicked: Option<TrackType>,
    /// Inline rename committed: (track_id, new_name).
    pub rename_commit: Option<(TrackId, String)>,
    /// Inline rename cancelled (Esc).
    pub rename_cancel: bool,
}

/// Preset color palette for the track-header color picker.
const TRACK_COLORS: &[(&str, Color)] = &[
    ("Gray",    Color { r: 140, g: 140, b: 140 }),
    ("Red",     Color { r: 244, g:  67, b:  54 }),
    ("Orange",  Color { r: 255, g: 152, b:   0 }),
    ("Yellow",  Color { r: 255, g: 193, b:   7 }),
    ("Green",   Color { r:  76, g: 175, b:  80 }),
    ("Teal",    Color { r:   0, g: 150, b: 136 }),
    ("Blue",    Color { r:  33, g: 150, b: 243 }),
    ("Purple",  Color { r: 156, g:  39, b: 176 }),
];

impl<'a> SessionView<'a> {
    /// Render the view and return the accumulated user interactions for this frame.
    pub fn show(mut self, ui: &mut egui::Ui) -> SessionViewResponse {
        let mut response = SessionViewResponse::default();

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let slot_width = 100.0;
                let slot_height = 40.0;
                let header_height = 30.0;
                let scene_button_width = 60.0;

                // Track headers
                ui.horizontal(|ui| {
                    // Empty corner for scene buttons
                    ui.allocate_space(egui::vec2(scene_button_width, header_height));

                    for track in &self.vm.track_headers {
                        let is_renaming = self.rename
                            .as_ref()
                            .map(|(id, _)| *id == track.id)
                            .unwrap_or(false);

                        if is_renaming {
                            // Extract the buffer so we can ergonomically pass &mut to TextEdit
                            // without fighting the borrow checker. `take()` is OK because we
                            // re-place the buffer through response.rename_commit if needed
                            // (the buffer is owned by OndeksApp; losing our local mut ref is fine).
                            if let Some((_, buf)) = self.rename.as_mut() {
                                let edit = egui::TextEdit::singleline(*buf)
                                    .desired_width(slot_width - 4.0)
                                    .min_size(egui::vec2(slot_width, header_height));
                                let edit_resp = ui.add(edit);
                                edit_resp.request_focus();

                                let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
                                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));

                                if escape {
                                    response.rename_cancel = true;
                                } else if enter || edit_resp.lost_focus() {
                                    response.rename_commit = Some((track.id, (*buf).clone()));
                                }
                                continue;
                            }
                        }

                        let (rect, header_resp) = ui.allocate_exact_size(
                            egui::vec2(slot_width, header_height),
                            egui::Sense::click_and_drag(),
                        );

                        // Track header background
                        let bg_color = if header_resp.hovered() {
                            egui::Color32::from_gray(60)
                        } else {
                            egui::Color32::from_gray(50)
                        };
                        ui.painter().rect_filled(rect, 2.0, bg_color);

                        // Track color indicator
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(rect.min, egui::vec2(4.0, header_height)),
                            0.0,
                            color_to_egui(track.color),
                        );

                        // Track name
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            &track.name,
                            egui::FontId::proportional(11.0),
                            egui::Color32::WHITE,
                        );

                        // Right-click context menu.
                        let mut picked: Option<TrackMenuAction> = None;
                        header_resp.context_menu(|ui| {
                            if ui.button("Rename").clicked() {
                                picked = Some(TrackMenuAction::StartRename);
                                ui.close_menu();
                            }
                            if ui.button("Duplicate").clicked() {
                                picked = Some(TrackMenuAction::Duplicate);
                                ui.close_menu();
                            }
                            if ui.button("Delete").clicked() {
                                picked = Some(TrackMenuAction::Delete);
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui.button("Move Up").clicked() {
                                picked = Some(TrackMenuAction::MoveUp);
                                ui.close_menu();
                            }
                            if ui.button("Move Down").clicked() {
                                picked = Some(TrackMenuAction::MoveDown);
                                ui.close_menu();
                            }
                            ui.separator();
                            ui.menu_button("Color", |ui| {
                                for (name, c) in TRACK_COLORS {
                                    if ui.button(*name).clicked() {
                                        picked = Some(TrackMenuAction::SetColor(*c));
                                        ui.close_menu();
                                    }
                                }
                            });
                        });
                        if let Some(action) = picked {
                            response.track_menu_action = Some((track.id, action));
                        }
                    }

                    // Add-track "+" button with MIDI / Audio submenu.
                    ui.menu_button("+ Track", |ui| {
                        if ui.button("MIDI").clicked() {
                            response.add_track_clicked = Some(TrackType::Midi);
                            ui.close_menu();
                        }
                        if ui.button("Audio").clicked() {
                            response.add_track_clicked = Some(TrackType::Audio);
                            ui.close_menu();
                        }
                    });

                    ui.separator();

                    // Stop all button
                    if ui.button("■ Stop All").clicked() {
                        response.stop_all_clicked = true;
                    }
                });

                ui.separator();

                // Scene rows
                for (scene_idx, _scene_name) in self.vm.scene_names.iter().enumerate() {
                    ui.horizontal(|ui| {
                        // Scene launch button
                        let scene_btn = ui.add_sized(
                            egui::vec2(scene_button_width, slot_height),
                            egui::Button::new(format!("▶ {}", self.vm.scene_names[scene_idx])),
                        );
                        if scene_btn.clicked() {
                            response.scene_launched = Some(scene_idx);
                        }

                        // Clip slots for this scene
                        if let Some(row) = self.vm.slots.get(scene_idx) {
                            for slot in row {
                                let slot_response = self.draw_slot(ui, slot, slot_width, slot_height);
                                if slot_response.clicked() {
                                    response.slot_clicked = Some((slot.track_index, slot.scene_index));
                                }
                            }
                        }
                    });
                }

                // Track stop buttons row
                ui.horizontal(|ui| {
                    ui.allocate_space(egui::vec2(scene_button_width, slot_height / 2.0));

                    for track in &self.vm.track_headers {
                        let btn = ui.add_sized(
                            egui::vec2(slot_width, slot_height / 2.0),
                            egui::Button::new("■ Stop"),
                        );
                        if btn.clicked() {
                            response.track_stopped = Some(track.index);
                        }
                    }
                });
            });

        response
    }

    /// Draw a single clip slot.
    fn draw_slot(&self, ui: &mut egui::Ui, slot: &SlotViewModel, width: f32, height: f32) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(width, height),
            egui::Sense::click(),
        );

        let painter = ui.painter();

        // Slot background based on state
        let bg_color = match slot.state_color_hint() {
            SlotColorHint::Dim => egui::Color32::from_gray(30),
            SlotColorHint::Normal => egui::Color32::from_gray(45),
            SlotColorHint::Active => egui::Color32::from_rgb(76, 175, 80).gamma_multiply(0.3),
            SlotColorHint::Warning => egui::Color32::from_rgb(255, 193, 7).gamma_multiply(0.3),
            SlotColorHint::Recording => egui::Color32::from_rgb(244, 67, 54).gamma_multiply(0.3),
        };

        painter.rect_filled(rect.shrink(1.0), 3.0, bg_color);

        // Selection highlight
        let is_selected = self.selection.is_selected(&SelectableItem::SessionSlot {
            track: slot.track_index,
            scene: slot.scene_index,
        });
        if is_selected {
            painter.rect_stroke(
                rect.shrink(1.0),
                3.0,
                egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 140, 0)),
            );
        }

        // Hover effect
        if response.hovered() {
            painter.rect_stroke(
                rect.shrink(1.0),
                3.0,
                egui::Stroke::new(1.0, egui::Color32::WHITE),
            );
        }

        // Slot content
        if let Some(clip) = &slot.clip {
            // Clip color bar
            painter.rect_filled(
                egui::Rect::from_min_size(
                    rect.min + egui::vec2(2.0, 2.0),
                    egui::vec2(4.0, height - 4.0),
                ),
                0.0,
                color_to_egui(clip.color),
            );

            // Clip name
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &clip.name,
                egui::FontId::proportional(11.0),
                egui::Color32::WHITE,
            );

            // State icon
            let state_icon = slot.state_icon();
            let icon_color = match slot.state {
                SlotState::Playing => egui::Color32::from_rgb(76, 175, 80),
                SlotState::Queued => egui::Color32::from_rgb(255, 193, 7),
                SlotState::Recording => egui::Color32::from_rgb(244, 67, 54),
                _ => egui::Color32::GRAY,
            };
            painter.text(
                rect.right_top() + egui::vec2(-12.0, 10.0),
                egui::Align2::CENTER_CENTER,
                state_icon,
                egui::FontId::proportional(12.0),
                icon_color,
            );
        } else {
            // Empty slot indicator
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "○",
                egui::FontId::proportional(16.0),
                egui::Color32::from_gray(80),
            );
        }

        response
    }
}

/// Convert ondeks Color to egui Color32.
fn color_to_egui(color: ondeks_core::Color) -> egui::Color32 {
    egui::Color32::from_rgb(color.r, color.g, color.b)
}
