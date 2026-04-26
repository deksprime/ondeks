//! Transport controls widget.

use eframe::egui;
use ondeks_ui_common::TransportViewModel;

/// Transport control buttons.
pub struct TransportControls<'a> {
    vm: &'a TransportViewModel,
}

impl<'a> TransportControls<'a> {
    pub fn new(vm: &'a TransportViewModel) -> Self {
        Self { vm }
    }

    /// Show the transport controls and return the response.
    pub fn show(self, ui: &mut egui::Ui) -> TransportResponse {
        let mut response = TransportResponse::default();

        ui.horizontal(|ui| {
            // Go to start
            if ui.button("⏮").on_hover_text("Go to Start").clicked() {
                response.goto_start = true;
            }

            // Play/Pause
            let play_btn = if self.vm.is_playing {
                ui.button("⏸").on_hover_text("Pause")
            } else {
                ui.button("▶").on_hover_text("Play")
            };
            if play_btn.clicked() {
                response.play_clicked = true;
            }

            // Stop
            if ui.button("⏹").on_hover_text("Stop").clicked() {
                response.stop_clicked = true;
            }

            // Record
            let record_color = if self.vm.is_recording {
                egui::Color32::from_rgb(244, 67, 54)
            } else {
                egui::Color32::GRAY
            };
            if ui.add(egui::Button::new("⏺").fill(record_color))
                .on_hover_text("Record")
                .clicked()
            {
                response.record_clicked = true;
            }

            ui.separator();

            // Loop toggle
            let loop_color = if self.vm.loop_enabled {
                egui::Color32::from_rgb(255, 140, 0)
            } else {
                egui::Color32::GRAY
            };
            if ui.add(egui::Button::new("🔁").fill(loop_color))
                .on_hover_text("Toggle Loop")
                .clicked()
            {
                response.loop_toggled = true;
            }

            // Metronome toggle
            let metro_color = if self.vm.metronome_enabled {
                egui::Color32::from_rgb(255, 140, 0)
            } else {
                egui::Color32::GRAY
            };
            if ui.add(egui::Button::new("🎵").fill(metro_color))
                .on_hover_text("Toggle Metronome")
                .clicked()
            {
                response.metronome_toggled = true;
            }
        });

        response
    }
}

/// Response from transport controls.
#[derive(Debug, Default)]
pub struct TransportResponse {
    pub play_clicked: bool,
    pub stop_clicked: bool,
    pub record_clicked: bool,
    pub loop_toggled: bool,
    pub metronome_toggled: bool,
    pub goto_start: bool,
    pub goto_end: bool,
}

/// Position display with scrubbing.
pub struct PositionDisplay<'a> {
    vm: &'a TransportViewModel,
    show_time: bool,
}

impl<'a> PositionDisplay<'a> {
    pub fn new(vm: &'a TransportViewModel) -> Self {
        Self { vm, show_time: false }
    }

    pub fn show_time(mut self, show: bool) -> Self {
        self.show_time = show;
        self
    }
}

impl egui::Widget for PositionDisplay<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let text = if self.show_time {
            self.vm.time_string()
        } else {
            self.vm.position_string()
        };

        let response = ui.add(
            egui::Label::new(
                egui::RichText::new(text)
                    .monospace()
                    .size(16.0)
            ).sense(egui::Sense::click())
        );

        response
    }
}

/// Tempo editor.
pub struct TempoEditor<'a> {
    tempo: &'a mut f64,
}

impl<'a> TempoEditor<'a> {
    pub fn new(tempo: &'a mut f64) -> Self {
        Self { tempo }
    }
}

impl egui::Widget for TempoEditor<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.horizontal(|ui| {
            ui.label("BPM");
            ui.add(
                egui::DragValue::new(self.tempo)
                    .speed(0.1)
                    .range(20.0..=999.0)
                    .fixed_decimals(1)
            )
        }).response
    }
}
