//! Main application state and update loop.

use eframe::egui;
use ondeks_runtime::Host;
use ondeks_ui_common::{
    Preferences, Selection, SelectableItem, History, HistoryEntry, UiCommand,
    TransportViewModel, ProjectViewModel, SessionViewModel, MeterViewModel,
    apply_project_command,
    commands::ProjectCommand,
};
use ondeks_core::TrackId;
use ondeks_core::project::{Project, TrackType};
use ondeks_core::session::SlotState;
use ondeks_core::transport::Transport;
use crate::views::{SessionView, TrackMenuAction};
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

    // Project state — UI-thread source of truth (Slice 4). Migrated to the
    // engine in Slice 5 when per-track instruments come online.
    project: Project,

    // UI state
    current_view: CurrentView,
    selection: Selection,
    history: History,
    #[allow(dead_code)]
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

    /// Inline-rename buffer for the session track header. When Some, the
    /// session view renders a TextEdit on this track; on commit or cancel the
    /// app dispatches a `RenameTrack` and clears the buffer.
    rename_buffer: Option<(TrackId, String)>,
}

impl OndeksApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Configure egui style
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        cc.egui_ctx.set_style(style);

        let mut host = Host::new().expect("Failed to create audio host");

        match host.start() {
            Ok(()) => {
                tracing::info!("Audio host started successfully");
            }
            Err(e) => {
                tracing::warn!("Failed to start audio host: {} - meters and transport updates may not work", e);
            }
        }

        // Slice 4: project opens empty — just master + Scene 1. User adds
        // tracks via the "+" button in the session view.
        let project = Project::new("New Project");

        let transport = Transport::new(44100);
        let transport_vm = TransportViewModel::from_transport(&transport, false);
        let project_vm = ProjectViewModel::from_project(&project);
        let session_vm = SessionViewModel::from_project(
            &project,
            |_track, _scene| SlotState::Empty,
            |_track, _scene| false,
        );

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
            rename_buffer: None,
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

    /// Rebuild project and session view models from the current project.
    fn rebuild_view_models(&mut self) {
        self.project_vm = ProjectViewModel::from_project(&self.project);
        self.session_vm = SessionViewModel::from_project(
            &self.project,
            |_track, _scene| SlotState::Empty,
            |_track, _scene| false,
        );
    }

    /// Apply a user-originated project command, record its inverse in the
    /// undo history, forward any engine commands to the audio thread, and
    /// rebuild view models. Arm commands are applied without history since
    /// arm is an ephemeral routing flag. Failed mutations are logged and
    /// leave state untouched.
    fn dispatch_project_command(&mut self, cmd: ProjectCommand) {
        let description = cmd.description().to_string();
        let is_undoable = !matches!(cmd, ProjectCommand::ArmTrack { .. });
        match apply_project_command(&mut self.project, &cmd) {
            Ok(outcome) => {
                for engine_cmd in &outcome.engine_commands {
                    let _ = self.host.send_command(engine_cmd.clone());
                }
                if is_undoable {
                    self.history.record(HistoryEntry {
                        description,
                        undo: UiCommand::Project(outcome.undo),
                        redo: UiCommand::Project(outcome.redo),
                    });
                }
                self.rebuild_view_models();
            }
            Err(e) => tracing::warn!("project dispatch failed ({description}): {e}"),
        }
    }

    /// Replay a UI command during undo/redo — mutate the project and forward
    /// any engine commands that mirror the mutation. History has the inverse
    /// pair already, so we don't push a new entry.
    fn apply_ui_command_replay(&mut self, cmd: UiCommand) {
        match cmd {
            UiCommand::Project(pc) => {
                match apply_project_command(&mut self.project, &pc) {
                    Ok(outcome) => {
                        for engine_cmd in &outcome.engine_commands {
                            let _ = self.host.send_command(engine_cmd.clone());
                        }
                    }
                    Err(e) => tracing::warn!("undo/redo replay failed: {e}"),
                }
                self.rebuild_view_models();
            }
            _ => {
                // Other UI command families aren't routed through the project
                // dispatcher yet; silently ignore for now.
            }
        }
    }

    /// Handle Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z shortcuts.
    ///
    /// Walks the event queue explicitly rather than using `consume_shortcut`
    /// because egui's `Modifiers::matches_logically` treats pattern modifiers
    /// as a *subset* requirement — a `Ctrl+Z` pattern also matches `Ctrl+Shift+Z`,
    /// which would swallow the redo event before redo's pattern could see it.
    /// Matching events by exact modifier equality sidesteps that.
    fn handle_undo_redo_shortcuts(&mut self, ctx: &egui::Context) {
        let (do_undo, do_redo) = ctx.input_mut(|i| {
            let mut do_undo = false;
            let mut do_redo = false;
            i.events.retain(|event| {
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                {
                    let cmd = modifiers.command || modifiers.ctrl;
                    let shift = modifiers.shift;
                    let alt = modifiers.alt;
                    if cmd && !alt {
                        // Ctrl+Z (no shift) → undo
                        if !shift && *key == egui::Key::Z {
                            do_undo = true;
                            return false;
                        }
                        // Ctrl+Shift+Z or Ctrl+Y → redo
                        if (shift && *key == egui::Key::Z)
                            || (!shift && *key == egui::Key::Y)
                        {
                            do_redo = true;
                            return false;
                        }
                    }
                }
                true
            });
            (do_undo, do_redo)
        });

        if do_undo {
            if let Some(cmd) = self.history.undo() {
                tracing::debug!("undo: {}", cmd.description());
                self.apply_ui_command_replay(cmd);
            }
        }
        if do_redo {
            if let Some(cmd) = self.history.redo() {
                tracing::debug!("redo: {}", cmd.description());
                self.apply_ui_command_replay(cmd);
            } else {
                tracing::debug!("redo shortcut fired but redo stack is empty");
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

            // Undo/redo status indicators
            ui.separator();
            ui.add_enabled(
                self.history.can_undo(),
                egui::Label::new(
                    egui::RichText::new(format!(
                        "↶ {}",
                        self.history.undo_description().unwrap_or("")
                    ))
                    .small()
                    .color(egui::Color32::from_gray(140)),
                ),
            );
            ui.add_enabled(
                self.history.can_redo(),
                egui::Label::new(
                    egui::RichText::new(format!(
                        "↷ {}",
                        self.history.redo_description().unwrap_or("")
                    ))
                    .small()
                    .color(egui::Color32::from_gray(140)),
                ),
            );

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
        // If a rename is in progress, hand the buffer to the session view so
        // it can render the TextEdit inline. This is ergonomic once the
        // borrow is carved out up front.
        let rename_state = self
            .rename_buffer
            .as_mut()
            .map(|(id, buf)| (*id, buf));

        let view = SessionView::new(&self.session_vm, &self.selection);
        let view = match rename_state {
            Some((id, buf)) => view.with_rename(id, buf),
            None => view,
        };
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

        // --- Track header actions ---
        if let Some(track_type) = response.add_track_clicked {
            let default_name = match track_type {
                TrackType::Midi => "MIDI Track",
                TrackType::Audio => "Audio Track",
                TrackType::Group => "Group",
                TrackType::Return => "Return",
                TrackType::Master => "Master",
            };
            self.dispatch_project_command(ProjectCommand::AddTrack {
                track_type,
                name: default_name.to_string(),
            });
        }

        if let Some((id, action)) = response.track_menu_action {
            match action {
                TrackMenuAction::StartRename => {
                    let current = self
                        .project
                        .get_track(id)
                        .map(|t| t.name.clone())
                        .unwrap_or_default();
                    self.rename_buffer = Some((id, current));
                }
                TrackMenuAction::Duplicate => {
                    self.dispatch_project_command(ProjectCommand::DuplicateTrack { track_id: id });
                }
                TrackMenuAction::Delete => {
                    self.dispatch_project_command(ProjectCommand::RemoveTrack { track_id: id });
                }
                TrackMenuAction::MoveUp => {
                    if let Some(idx) = self.project.track_index(id) {
                        if idx > 0 {
                            self.dispatch_project_command(ProjectCommand::MoveTrack {
                                track_id: id,
                                new_index: idx - 1,
                            });
                        }
                    }
                }
                TrackMenuAction::MoveDown => {
                    if let Some(idx) = self.project.track_index(id) {
                        self.dispatch_project_command(ProjectCommand::MoveTrack {
                            track_id: id,
                            new_index: idx + 1,
                        });
                    }
                }
                TrackMenuAction::SetColor(color) => {
                    self.dispatch_project_command(ProjectCommand::SetTrackColor {
                        track_id: id,
                        color,
                    });
                }
            }
        }

        if let Some((id, new_name)) = response.rename_commit {
            let trimmed = new_name.trim();
            if !trimmed.is_empty() {
                self.dispatch_project_command(ProjectCommand::RenameTrack {
                    track_id: id,
                    name: trimmed.to_string(),
                });
            }
            self.rename_buffer = None;
        }
        if response.rename_cancel {
            self.rename_buffer = None;
        }

        if let Some(track_id) = response.arm_track {
            // Toggle: if this track is already armed, clicking disarms it.
            let already_armed = self
                .project
                .get_track(track_id)
                .map(|t| t.armed)
                .unwrap_or(false);
            if already_armed {
                self.project.disarm_all();
                self.rebuild_view_models();
            } else {
                self.dispatch_project_command(ProjectCommand::ArmTrack { track_id });
            }
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

    /// Render the on-screen piano keyboard. Forwards MIDI events to the
    /// armed MIDI track's instrument. Dropped silently if no track is armed
    /// or the armed track has no instrument.
    fn render_keyboard(&mut self, ui: &mut egui::Ui) {
        let response = PianoKeyboard::new().height(90.0).show(ui);
        if response.events.is_empty() {
            return;
        }
        let target = self
            .project
            .armed_track()
            .and_then(|t| t.instrument);
        let Some(target) = target else {
            // No armed MIDI track → keyboard input is silent. Tracing only,
            // no user-visible warning (common during startup / between arms).
            return;
        };
        for event in response.events {
            let _ = self.host.send_command(ondeks_core::Command::SendMidi {
                target,
                event,
                sample_offset: 0,
            });
        }
    }

    /// Render the mixer panel: per-track strips + master.
    fn render_mixer(&mut self, ui: &mut egui::Ui) {
        ui.heading("Mixer");

        // Collect actions inside the iteration so we don't double-borrow self
        // for both view models (read) and dispatchers (write).
        let mut actions: Vec<(TrackId, StripActionKind)> = Vec::new();
        let mut master_volume_change: Option<f32> = None;

        ui.horizontal(|ui| {
            // Per-track strips (skip master — rendered separately on the right).
            for track in self
                .project_vm
                .tracks
                .iter()
                .filter(|t| t.track_type != TrackType::Master)
            {
                ui.vertical(|ui| {
                    ui.set_width(64.0);

                    // Color stripe + name.
                    let (stripe_rect, _) = ui.allocate_exact_size(
                        egui::vec2(60.0, 4.0),
                        egui::Sense::hover(),
                    );
                    ui.painter().rect_filled(
                        stripe_rect,
                        1.0,
                        egui::Color32::from_rgb(track.color.r, track.color.g, track.color.b),
                    );
                    ui.label(
                        egui::RichText::new(&track.name)
                            .size(11.0)
                            .color(egui::Color32::from_gray(220)),
                    );

                    // Fader (vertical).
                    let mut volume = track.volume_db;
                    if ui
                        .add(
                            egui::Slider::new(&mut volume, -60.0..=6.0)
                                .vertical()
                                .show_value(false)
                                .text(""),
                        )
                        .changed()
                    {
                        actions.push((track.id, StripActionKind::SetVolume(volume)));
                    }
                    ui.label(
                        egui::RichText::new(track.volume_string())
                            .size(10.0)
                            .color(egui::Color32::from_gray(180)),
                    );

                    // Pan slider (horizontal, narrow).
                    let mut pan = track.pan;
                    if ui
                        .add(
                            egui::Slider::new(&mut pan, -1.0..=1.0)
                                .show_value(false)
                                .text(""),
                        )
                        .changed()
                    {
                        actions.push((track.id, StripActionKind::SetPan(pan)));
                    }
                    ui.label(
                        egui::RichText::new(track.pan_string())
                            .size(10.0)
                            .color(egui::Color32::from_gray(180)),
                    );

                    // Mute/Solo buttons.
                    ui.horizontal(|ui| {
                        let m_color = if track.muted {
                            egui::Color32::from_rgb(255, 193, 7)
                        } else {
                            egui::Color32::from_gray(70)
                        };
                        if ui
                            .add(
                                egui::Button::new("M")
                                    .fill(m_color)
                                    .min_size(egui::vec2(22.0, 18.0)),
                            )
                            .clicked()
                        {
                            actions.push((track.id, StripActionKind::ToggleMute));
                        }

                        let s_color = if track.soloed {
                            egui::Color32::from_rgb(76, 175, 80)
                        } else {
                            egui::Color32::from_gray(70)
                        };
                        if ui
                            .add(
                                egui::Button::new("S")
                                    .fill(s_color)
                                    .min_size(egui::vec2(22.0, 18.0)),
                            )
                            .clicked()
                        {
                            actions.push((track.id, StripActionKind::ToggleSolo));
                        }
                    });
                });

                ui.separator();
            }

            // Master strip on the right.
            if let Some(master_vm) = self.project_vm.master_track() {
                ui.vertical(|ui| {
                    ui.set_width(72.0);
                    ui.label(
                        egui::RichText::new("Master")
                            .strong()
                            .color(egui::Color32::from_gray(230)),
                    );
                    let mut master_volume = master_vm.volume_db;
                    if ui
                        .add(
                            egui::Slider::new(&mut master_volume, -60.0..=6.0)
                                .vertical()
                                .show_value(false)
                                .text(""),
                        )
                        .changed()
                    {
                        master_volume_change = Some(master_volume);
                    }
                    ui.label(
                        egui::RichText::new(master_vm.volume_string())
                            .size(10.0)
                            .color(egui::Color32::from_gray(180)),
                    );
                });
            }
        });

        // Apply actions outside the borrow scope.
        for (track_id, kind) in actions {
            self.apply_strip_action(track_id, kind);
        }
        if let Some(v) = master_volume_change {
            self.set_master_volume(v);
        }
    }

    fn apply_strip_action(&mut self, track_id: TrackId, kind: StripActionKind) {
        // Read current state once.
        let Some(track) = self.project.get_track(track_id) else { return };
        let strip_id = track.channel_strip;
        let cur_muted = track.muted;
        let cur_soloed = track.soloed;

        match kind {
            StripActionKind::SetVolume(v) => {
                if let Some(t) = self.project.get_track_mut(track_id) {
                    t.volume_db = v;
                }
                if let Some(s) = strip_id {
                    let _ = self.host.send_command(ondeks_core::Command::SetTrackVolume {
                        node_id: s,
                        volume_db: v,
                    });
                }
            }
            StripActionKind::SetPan(p) => {
                if let Some(t) = self.project.get_track_mut(track_id) {
                    t.pan = p;
                }
                if let Some(s) = strip_id {
                    let _ = self.host.send_command(ondeks_core::Command::SetTrackPan {
                        node_id: s,
                        pan: p,
                    });
                }
            }
            StripActionKind::ToggleMute => {
                let new_muted = !cur_muted;
                if let Some(t) = self.project.get_track_mut(track_id) {
                    t.muted = new_muted;
                }
                if let Some(s) = strip_id {
                    let _ = self.host.send_command(ondeks_core::Command::SetTrackMute {
                        node_id: s,
                        muted: new_muted,
                    });
                }
            }
            StripActionKind::ToggleSolo => {
                let new_soloed = !cur_soloed;
                if let Some(t) = self.project.get_track_mut(track_id) {
                    t.soloed = new_soloed;
                }
                if let Some(s) = strip_id {
                    let _ = self.host.send_command(ondeks_core::Command::SetTrackSolo {
                        node_id: s,
                        soloed: new_soloed,
                    });
                }
            }
        }
        self.rebuild_view_models();
    }

    fn set_master_volume(&mut self, volume_db: f32) {
        self.project.master_mut().volume_db = volume_db;
        let _ = self
            .host
            .send_command(ondeks_core::Command::SetMasterVolume { volume_db });
        self.rebuild_view_models();
    }
}

/// Mixer strip action types (declared at module scope so the inner helper can
/// take them by value without inheriting the closure's borrow scope).
enum StripActionKind {
    SetVolume(f32),
    SetPan(f32),
    ToggleMute,
    ToggleSolo,
}

impl eframe::App for OndeksApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll for runtime events
        self.poll_runtime_events();

        // Undo / redo shortcuts
        self.handle_undo_redo_shortcuts(ctx);

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
