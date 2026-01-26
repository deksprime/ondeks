//! Main application state and update loop.

use eframe::egui;
use ondeks_runtime::Host;
use ondeks_ui_common::{
    Theme, Preferences, Selection, History,
    TransportViewModel, ProjectViewModel, MeterViewModel,
};
use ondeks_core::project::Project;
use ondeks_core::transport::Transport;

/// The view currently displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CurrentView {
    #[default]
    Session,
    Arrangement,
}

/// Main application state.
pub struct OndeksApp {
    // Runtime
    host: Host,

    // Project state
    project: Project,

    // UI state
    current_view: CurrentView,
    selection: Selection,
    history: History,
    preferences: Preferences,

    // View models (updated each frame)
    transport_vm: TransportViewModel,
    project_vm: ProjectViewModel,
    master_meters: MeterViewModel,

    // UI state
    show_mixer: bool,
    show_inspector: bool,
}

impl OndeksApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Configure egui style
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        cc.egui_ctx.set_style(style);

        // Initialize runtime
        let mut host = Host::new().expect("Failed to create audio host");
        
        // Start the audio host
        // Note: We'll handle errors gracefully in production
        match host.start() {
            Ok(()) => {
                tracing::info!("Audio host started successfully");
            }
            Err(e) => {
                tracing::warn!("Failed to start audio host: {} - meters and transport updates may not work", e);
            }
        }

        // Initialize project
        let project = Project::new("New Project");

        // Create initial view models
        let transport = Transport::new(44100);
        let transport_vm = TransportViewModel::from_transport(&transport, false);
        let project_vm = ProjectViewModel::from_project(&project);

        Self {
            host,
            project,
            current_view: CurrentView::Session,
            selection: Selection::new(),
            history: History::new(100),
            preferences: Preferences::default(),
            transport_vm,
            project_vm,
            master_meters: MeterViewModel::default(),
            show_mixer: true,
            show_inspector: true,
        }
    }

    /// Poll for events from the audio runtime.
    fn poll_runtime_events(&mut self) {
        for event in self.host.poll_events() {
            match event {
                ondeks_runtime::queue::RuntimeEvent::MeterUpdate { left, right } => {
                    // Use a slower decay (0.95) so meters don't disappear too quickly
                    // Only decay if we're getting actual zero values (no audio)
                    if left > 0.0 || right > 0.0 {
                        self.master_meters.update(left, right, 0.98);
                    } else {
                        // If we get zeros, still decay but more slowly
                        self.master_meters.update(left, right, 0.95);
                    }
                }
                ondeks_runtime::queue::RuntimeEvent::PositionUpdate { beats, .. } => {
                    // Update transport view model position
                    // TODO: Get full transport state from runtime
                    self.transport_vm.position_beats = beats;
                }
                ondeks_runtime::queue::RuntimeEvent::TransportStateChanged { is_playing } => {
                    self.transport_vm.is_playing = is_playing;
                }
                _ => {}
            }
        }
    }

    /// Render the top toolbar.
    fn render_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // View toggle buttons
            ui.selectable_value(&mut self.current_view, CurrentView::Session, "Session");
            ui.selectable_value(&mut self.current_view, CurrentView::Arrangement, "Arrangement");

            ui.separator();

            // Transport controls
            if ui.button("⏮").clicked() {
                let _ = self.host.send_command(ondeks_core::Command::Stop);
            }

            let play_text = if self.transport_vm.is_playing { "⏸" } else { "▶" };
            if ui.button(play_text).clicked() {
                if self.transport_vm.is_playing {
                    let _ = self.host.send_command(ondeks_core::Command::Stop);
                } else {
                    let _ = self.host.send_command(ondeks_core::Command::Play);
                }
            }

            if ui.button("⏹").clicked() {
                let _ = self.host.send_command(ondeks_core::Command::Stop);
            }

            if ui.button("⏺").on_hover_text("Record").clicked() {
                // TODO: Toggle recording
            }

            ui.separator();

            // Position display
            ui.monospace(&self.transport_vm.position_string());

            ui.separator();

            // Tempo
            ui.label("BPM:");
            let mut tempo = self.transport_vm.tempo;
            if ui.add(egui::DragValue::new(&mut tempo).speed(0.1).range(20.0..=999.0)).changed() {
                // Update local view model immediately for responsive UI
                self.transport_vm.tempo = tempo;
                // Send command to runtime
                let _ = self.host.send_command(ondeks_core::Command::SetTempo(tempo));
            }

            ui.separator();

            // Time signature
            ui.label(&self.transport_vm.time_sig_string());

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Right side controls
                ui.toggle_value(&mut self.show_mixer, "Mixer");
                ui.toggle_value(&mut self.show_inspector, "Inspector");

                // Master meters (simple bars)
                let meter_height = 12.0;
                let meter_width = 100.0;
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(meter_width, meter_height),
                    egui::Sense::hover(),
                );

                let left_width = (self.master_meters.left_normalized() * meter_width).min(meter_width);
                let right_width = (self.master_meters.right_normalized() * meter_width).min(meter_width);

                ui.painter().rect_filled(
                    rect,
                    2.0,
                    egui::Color32::from_gray(40),
                );
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(rect.min, egui::vec2(left_width, meter_height / 2.0 - 1.0)),
                    0.0,
                    egui::Color32::from_rgb(76, 175, 80),
                );
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(
                        rect.min + egui::vec2(0.0, meter_height / 2.0 + 1.0),
                        egui::vec2(right_width, meter_height / 2.0 - 1.0)
                    ),
                    0.0,
                    egui::Color32::from_rgb(76, 175, 80),
                );
            });
        });
    }

    /// Render the main content area.
    fn render_main_content(&mut self, ui: &mut egui::Ui) {
        match self.current_view {
            CurrentView::Session => self.render_session_view(ui),
            CurrentView::Arrangement => self.render_arrangement_view(ui),
        }
    }

    /// Render session view (clip launcher grid).
    fn render_session_view(&mut self, ui: &mut egui::Ui) {
        ui.heading("Session View");
        ui.label("Clip launcher grid will be implemented in Phase 2.3");

        // Placeholder grid
        egui::Grid::new("session_grid")
            .num_columns(self.project_vm.tracks.len().max(1))
            .spacing([4.0, 4.0])
            .show(ui, |ui| {
                // Track headers
                for track in &self.project_vm.tracks {
                    ui.label(&track.name);
                }
                ui.end_row();

                // Scene rows
                for scene in &self.project_vm.scenes {
                    for _track in &self.project_vm.tracks {
                        if ui.button("○").clicked() {
                            // TODO: Launch clip
                        }
                    }
                    ui.label(&scene.name);
                    ui.end_row();
                }
            });
    }

    /// Render arrangement view (timeline).
    fn render_arrangement_view(&mut self, ui: &mut egui::Ui) {
        ui.heading("Arrangement View");
        ui.label("Timeline editor will be implemented in Phase 2.4");

        // Placeholder timeline
        let available_size = ui.available_size();
        let (rect, _response) = ui.allocate_exact_size(available_size, egui::Sense::click_and_drag());

        // Draw background
        ui.painter().rect_filled(rect, 0.0, egui::Color32::from_gray(30));

        // Draw grid lines
        let beats_visible = 16;
        let beat_width = rect.width() / beats_visible as f32;
        for i in 0..=beats_visible {
            let x = rect.left() + i as f32 * beat_width;
            let color = if i % 4 == 0 {
                egui::Color32::from_gray(80)
            } else {
                egui::Color32::from_gray(50)
            };
            ui.painter().line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(1.0, color),
            );
        }

        // Draw track lanes
        let track_height = 60.0;
        for (i, track) in self.project_vm.tracks.iter().enumerate() {
            let y = rect.top() + i as f32 * track_height;
            if y > rect.bottom() {
                break;
            }

            // Track background
            let track_rect = egui::Rect::from_min_size(
                egui::pos2(rect.left(), y),
                egui::vec2(rect.width(), track_height - 1.0),
            );
            let bg_color = if i % 2 == 0 {
                egui::Color32::from_gray(35)
            } else {
                egui::Color32::from_gray(40)
            };
            ui.painter().rect_filled(track_rect, 0.0, bg_color);

            // Track name
            ui.painter().text(
                egui::pos2(rect.left() + 4.0, y + 4.0),
                egui::Align2::LEFT_TOP,
                &track.name,
                egui::FontId::default(),
                egui::Color32::from_gray(180),
            );
        }

        // Draw playhead
        let playhead_x = rect.left() + (self.transport_vm.position_beats as f32 / beats_visible as f32) * rect.width();
        if playhead_x >= rect.left() && playhead_x <= rect.right() {
            ui.painter().line_segment(
                [egui::pos2(playhead_x, rect.top()), egui::pos2(playhead_x, rect.bottom())],
                egui::Stroke::new(2.0, egui::Color32::from_rgb(76, 175, 80)),
            );
        }
    }

    /// Render the mixer panel.
    fn render_mixer(&mut self, ui: &mut egui::Ui) {
        ui.heading("Mixer");

        ui.horizontal(|ui| {
            for track in &self.project_vm.tracks {
                ui.vertical(|ui| {
                    ui.set_width(60.0);

                    // Track name
                    ui.label(&track.name);

                    // Fader (vertical slider)
                    let mut volume = track.volume_db;
                    ui.add(egui::Slider::new(&mut volume, -60.0..=6.0).vertical().text("dB"));

                    // Pan knob (placeholder)
                    ui.label(track.pan_string());

                    // Mute/Solo buttons
                    ui.horizontal(|ui| {
                        let m_color = if track.muted { egui::Color32::YELLOW } else { egui::Color32::GRAY };
                        if ui.add(egui::Button::new("M").fill(m_color).min_size(egui::vec2(20.0, 20.0))).clicked() {
                            // TODO: Toggle mute
                        }

                        let s_color = if track.soloed { egui::Color32::from_rgb(76, 175, 80) } else { egui::Color32::GRAY };
                        if ui.add(egui::Button::new("S").fill(s_color).min_size(egui::vec2(20.0, 20.0))).clicked() {
                            // TODO: Toggle solo
                        }
                    });
                });

                ui.separator();
            }
        });
    }
}

impl eframe::App for OndeksApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll for runtime events
        self.poll_runtime_events();

        // Request continuous repaints for meters and transport updates
        // Use a reasonable frame rate (60fps) instead of continuous
        ctx.request_repaint_after(std::time::Duration::from_millis(16));

        // Top toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            self.render_toolbar(ui);
        });

        // Bottom mixer panel (if visible)
        if self.show_mixer {
            egui::TopBottomPanel::bottom("mixer")
                .resizable(true)
                .min_height(100.0)
                .default_height(200.0)
                .show(ctx, |ui| {
                    self.render_mixer(ui);
                });
        }

        // Right inspector panel (if visible)
        if self.show_inspector {
            egui::SidePanel::right("inspector")
                .resizable(true)
                .min_width(200.0)
                .default_width(250.0)
                .show(ctx, |ui| {
                    ui.heading("Inspector");
                    ui.label("Selection details will appear here");

                    ui.separator();

                    // Project info
                    ui.label(format!("Project: {}", self.project_vm.name));
                    ui.label(format!("Tracks: {}", self.project_vm.tracks.len()));
                    ui.label(format!("Clips: {}", self.project_vm.clip_count));
                });
        }

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_main_content(ui);
        });
    }
}
