//! Session view (clip launcher grid).

use eframe::egui;
use ondeks_ui_common::{SessionViewModel, SlotViewModel, SlotColorHint, Selection, SelectableItem};
use ondeks_core::session::SlotState;

/// Session view widget.
pub struct SessionView<'a> {
    vm: &'a SessionViewModel,
    selection: &'a Selection,
}

impl<'a> SessionView<'a> {
    pub fn new(vm: &'a SessionViewModel, selection: &'a Selection) -> Self {
        Self { vm, selection }
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
}

impl<'a> SessionView<'a> {
    pub fn show(self, ui: &mut egui::Ui) -> SessionViewResponse {
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
                        let (rect, _track_response) = ui.allocate_exact_size(
                            egui::vec2(slot_width, header_height),
                            egui::Sense::click(),
                        );

                        // Track header background
                        let bg_color = egui::Color32::from_gray(50);
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
                    }

                    // Stop all button
                    if ui.button("■ Stop All").clicked() {
                        response.stop_all_clicked = true;
                    }
                });

                ui.separator();

                // Scene rows
                for (scene_idx, scene_name) in self.vm.scene_names.iter().enumerate() {
                    ui.horizontal(|ui| {
                        // Scene launch button
                        let scene_btn = ui.add_sized(
                            egui::vec2(scene_button_width, slot_height),
                            egui::Button::new(format!("▶ {}", scene_name)),
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
