//! Audio level meters.

use eframe::egui;
use ondeks_ui_common::MeterViewModel;

/// Vertical level meter.
pub struct LevelMeter<'a> {
    vm: &'a MeterViewModel,
    width: f32,
    height: f32,
}

impl<'a> LevelMeter<'a> {
    pub fn new(vm: &'a MeterViewModel) -> Self {
        Self {
            vm,
            width: 12.0,
            height: 100.0,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }
}

impl egui::Widget for LevelMeter<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(self.width * 2.0 + 2.0, self.height),
            egui::Sense::click(),
        );

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // Background
            painter.rect_filled(rect, 2.0, egui::Color32::from_gray(20));

            // Left meter
            let left_rect = egui::Rect::from_min_size(
                rect.min,
                egui::vec2(self.width, self.height),
            );
            draw_meter_bar(painter, left_rect, self.vm.left_normalized(), self.vm.is_clipping);

            // Right meter
            let right_rect = egui::Rect::from_min_size(
                rect.min + egui::vec2(self.width + 2.0, 0.0),
                egui::vec2(self.width, self.height),
            );
            draw_meter_bar(painter, right_rect, self.vm.right_normalized(), self.vm.is_clipping);

            // dB scale markings
            let marks = [-60, -48, -36, -24, -12, -6, 0, 6];
            for &db in &marks {
                let y = rect.top() + (1.0 - db_to_position(db as f32)) * self.height;
                painter.line_segment(
                    [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                    egui::Stroke::new(0.5, egui::Color32::from_gray(60)),
                );
            }
        }

        // Handle click to reset clip indicator
        if response.clicked() {
            // Signal that clip should be reset (handled by caller)
        }

        response
    }
}

/// Horizontal level meter (for mixer strips).
pub struct HorizontalMeter<'a> {
    vm: &'a MeterViewModel,
    width: f32,
    height: f32,
}

impl<'a> HorizontalMeter<'a> {
    pub fn new(vm: &'a MeterViewModel) -> Self {
        Self {
            vm,
            width: 100.0,
            height: 8.0,
        }
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }
}

impl egui::Widget for HorizontalMeter<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(self.width, self.height * 2.0 + 1.0),
            egui::Sense::hover(),
        );

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // Background
            painter.rect_filled(rect, 1.0, egui::Color32::from_gray(25));

            // Left channel
            let left_width = self.vm.left_normalized().min(1.2) * self.width;
            painter.rect_filled(
                egui::Rect::from_min_size(rect.min, egui::vec2(left_width, self.height)),
                0.0,
                meter_color(self.vm.left_db),
            );

            // Right channel
            let right_width = self.vm.right_normalized().min(1.2) * self.width;
            painter.rect_filled(
                egui::Rect::from_min_size(
                    rect.min + egui::vec2(0.0, self.height + 1.0),
                    egui::vec2(right_width, self.height),
                ),
                0.0,
                meter_color(self.vm.right_db),
            );

            // Clip indicator
            if self.vm.is_clipping {
                painter.rect_filled(
                    egui::Rect::from_min_size(
                        rect.right_top() - egui::vec2(4.0, 0.0),
                        egui::vec2(4.0, rect.height()),
                    ),
                    0.0,
                    egui::Color32::from_rgb(244, 67, 54),
                );
            }
        }

        response
    }
}

/// Draw a single vertical meter bar.
fn draw_meter_bar(painter: &egui::Painter, rect: egui::Rect, normalized: f32, clipping: bool) {
    let fill_height = normalized.min(1.2) * rect.height();
    let fill_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.bottom() - fill_height),
        rect.max,
    );

    // Gradient effect by drawing multiple sections
    let green_end = rect.bottom() - rect.height() * 0.6;  // -24dB
    let yellow_end = rect.bottom() - rect.height() * 0.9; // -6dB

    // Green section
    if fill_rect.bottom() > green_end {
        let section = egui::Rect::from_min_max(
            egui::pos2(rect.left(), green_end.max(fill_rect.top())),
            egui::pos2(rect.right(), fill_rect.bottom()),
        );
        painter.rect_filled(section, 0.0, egui::Color32::from_rgb(76, 175, 80));
    }

    // Yellow section
    if fill_rect.top() < green_end {
        let section = egui::Rect::from_min_max(
            egui::pos2(rect.left(), yellow_end.max(fill_rect.top())),
            egui::pos2(rect.right(), green_end.min(fill_rect.bottom())),
        );
        painter.rect_filled(section, 0.0, egui::Color32::from_rgb(255, 193, 7));
    }

    // Red section
    if fill_rect.top() < yellow_end {
        let section = egui::Rect::from_min_max(
            fill_rect.min,
            egui::pos2(rect.right(), yellow_end.min(fill_rect.bottom())),
        );
        painter.rect_filled(section, 0.0, egui::Color32::from_rgb(244, 67, 54));
    }

    // Clip indicator
    if clipping {
        let clip_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 4.0));
        painter.rect_filled(clip_rect, 0.0, egui::Color32::from_rgb(244, 67, 54));
    }
}

/// Convert dB to meter position (0.0 to 1.0+).
fn db_to_position(db: f32) -> f32 {
    ((db + 60.0) / 66.0).max(0.0) // -60dB to +6dB
}

/// Get color based on dB level.
fn meter_color(db: f32) -> egui::Color32 {
    if db > 0.0 {
        egui::Color32::from_rgb(244, 67, 54) // Red
    } else if db > -6.0 {
        egui::Color32::from_rgb(255, 193, 7) // Yellow
    } else {
        egui::Color32::from_rgb(76, 175, 80) // Green
    }
}
