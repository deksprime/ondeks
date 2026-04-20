//! Main application state and update loop.

use eframe::egui;
use ondeks_runtime::Host;
use ondeks_ui_common::{
    Preferences, Selection, SelectableItem, History,
    TransportViewModel, ProjectViewModel, SessionViewModel, MeterViewModel,
};
use ondeks_core::NodeId;
use ondeks_core::project::{Project, TrackType};
use ondeks_core::session::SlotState;
use ondeks_core::transport::Transport;
use crate::views::SessionView;
use crate::widgets::{TransportControls, PositionDisplay, TempoEditor, LevelMeter, PianoKeyboard};

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
    session_vm: SessionViewModel,
    master_meters: MeterViewModel,

    // UI state
    show_mixer: bool,
    show_inspector: bool,
    show_keyboard: bool,

    // Built-in synth node in the default graph. UI targets MIDI at this node
    // until Slice 4 replaces it with per-track instrument lookup.
    synth_node_id: NodeId,
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

        // Initialize project with demo tracks for testing
        let mut project = Project::new("New Project");
        project.add_track(TrackType::Midi, "Bass");
        project.add_track(TrackType::Midi, "Lead");
        project.add_track(TrackType::Audio, "Drums");
        project.add_track(TrackType::Audio, "Vocals");
        project.add_scene("Scene 2");
        project.add_scene("Scene 3");
        project.add_scene("Scene 4");

        // Create initial view models
        let transport = Transport::new(44100);
        let transport_vm = TransportViewModel::from_transport(&transport, false);
        let project_vm = ProjectViewModel::from_project(&project);
        let session_vm = SessionViewModel::from_project(
            &project,
            |_track, _scene| SlotState::Empty,
            |_track, _scene| false,
        );

        let synth_node_id = host.synth_node_id();

        Self {
            host,
            project,
            current_view: CurrentView::Session,
            selection: Selection::new(),
            history: History::new(100),
            preferences: Preferences::default(),
            transport_vm,
            project_vm,
            session_vm,
            master_meters: MeterViewModel::default(),
            show_mixer: true,
            show_inspector: true,
            show_keyboard: true,
            synth_node_id,
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
                ondeks_runtime::queue::RuntimeEvent::PositionUpdate { beats, samples, bbt } => {
                    // Update transport view model position with accurate values from audio thread
                    self.transport_vm.position_beats = beats;
                    self.transport_vm.position_seconds = samples as f64 / 44100.0; // TODO: Use actual sample rate
                    self.transport_vm.position_bbt = bbt;
                }
                ondeks_runtime::queue::RuntimeEvent::TransportStateChanged { is_playing } => {
                    tracing::info!("UI: Transport state changed - is_playing: {}", is_playing);
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

            // Transport controls widget
            let transport_controls = TransportControls::new(&self.transport_vm);
            let transport_response = transport_controls.show(ui);

            // Handle transport button clicks
            if transport_response.goto_start {
                tracing::info!("UI: Go to start clicked");
                let _ = self.host.send_command(ondeks_core::Command::Stop);
                // Also reset position to start
                self.transport_vm.position_beats = 0.0;
            }
            if transport_response.play_clicked {
                if self.transport_vm.is_playing {
                    tracing::info!("UI: Pause clicked (stopping playback)");
                    let _ = self.host.send_command(ondeks_core::Command::Stop);
                } else {
                    tracing::info!("UI: Play clicked");
                    let _ = self.host.send_command(ondeks_core::Command::Play);
                }
            }
            if transport_response.stop_clicked {
                tracing::info!("UI: Stop clicked");
                let _ = self.host.send_command(ondeks_core::Command::Stop);
            }
            if transport_response.record_clicked {
                // Toggle recording state (UI only for now)
                self.transport_vm.is_recording = !self.transport_vm.is_recording;
                tracing::info!("Recording toggled: {}", self.transport_vm.is_recording);
                // TODO: Send record command to backend when available
            }
            if transport_response.loop_toggled {
                // Toggle loop state
                self.transport_vm.loop_enabled = !self.transport_vm.loop_enabled;
                tracing::info!("Loop toggled: {}", self.transport_vm.loop_enabled);
                // TODO: Send loop command to backend when available
            }
            if transport_response.metronome_toggled {
                // Toggle metronome state
                self.transport_vm.metronome_enabled = !self.transport_vm.metronome_enabled;
                tracing::info!("Metronome toggled: {}", self.transport_vm.metronome_enabled);
                // TODO: Send metronome command to backend when available
            }

            ui.separator();

            // Position display widget
            let position_display = PositionDisplay::new(&self.transport_vm);
            let position_response = ui.add(position_display);
            if position_response.clicked() {
                // TODO: Toggle between time and beats display
            }

            ui.separator();

            // Tempo editor widget
            let mut tempo = self.transport_vm.tempo;
            let tempo_editor = TempoEditor::new(&mut tempo);
            if ui.add(tempo_editor).changed() {
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
                ui.toggle_value(&mut self.show_keyboard, "Keyboard");
                ui.toggle_value(&mut self.show_inspector, "Inspector");

                // Master meters using the LevelMeter widget
                let meter = LevelMeter::new(&self.master_meters)
                    .width(6.0)
                    .height(20.0);
                let meter_response = ui.add(meter);
                if meter_response.clicked() {
                    // Reset clip indicator
                    self.master_meters.reset_clip();
                }
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
        let view = SessionView::new(&self.session_vm, &self.selection);
        let response = view.show(ui);

        // Handle session view interactions
        if let Some((track, scene)) = response.slot_clicked {
            self.selection.select(SelectableItem::SessionSlot { track, scene });
            tracing::info!("Session slot clicked: track={}, scene={}", track, scene);
        }
        if let Some(scene) = response.scene_launched {
            tracing::info!("Scene launched: {}", scene);
        }
        if let Some(track) = response.track_stopped {
            tracing::info!("Track stopped: {}", track);
        }
        if response.stop_all_clicked {
            tracing::info!("Stop all clips");
        }
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

    /// Render the on-screen piano keyboard. Forwards generated MIDI events
    /// to the engine via `Command::SendMidi`.
    fn render_keyboard(&mut self, ui: &mut egui::Ui) {
        let response = PianoKeyboard::new().height(90.0).show(ui);
        for event in response.events {
            let _ = self.host.send_command(ondeks_core::Command::SendMidi {
                target: self.synth_node_id,
                event,
                sample_offset: 0,
            });
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

        // Bottom mixer panel (if visible) — docks at the very bottom.
        if self.show_mixer {
            egui::TopBottomPanel::bottom("mixer")
                .resizable(true)
                .min_height(100.0)
                .default_height(200.0)
                .show(ctx, |ui| {
                    self.render_mixer(ui);
                });
        }

        // Keyboard panel, stacked above the mixer (egui stacks bottom panels
        // in show() order — the first is outermost).
        if self.show_keyboard {
            egui::TopBottomPanel::bottom("keyboard")
                .resizable(false)
                .exact_height(100.0)
                .show(ctx, |ui| {
                    self.render_keyboard(ui);
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
