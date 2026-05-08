//! Main application state and update loop.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use eframe::egui;
use ondeks_runtime::Host;
use ondeks_ui_common::{
    Preferences, Selection, SelectableItem, History, HistoryEntry, UiCommand,
    TransportViewModel, ProjectViewModel, SessionViewModel, MeterViewModel,
    apply_project_command,
    commands::ProjectCommand,
};
use ondeks_core::{ClipId, TrackId};
use ondeks_core::midi::{Channel, MidiEvent, Note as MidiPitch, Velocity};
use ondeks_core::persistence::{project_from_json, project_to_json};
use ondeks_core::project::{Clip, MidiClip, MidiNote, Project, TrackType};
use ondeks_core::transport::Beats;
use ondeks_core::session::SlotState;
use ondeks_core::transport::Transport;
use crate::views::{DragState, PianoRollView, SessionView, TrackMenuAction};
use crate::widgets::{TransportControls, PositionDisplay, TempoEditor, LevelMeter, PianoKeyboard};

/// The view currently displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CurrentView {
    #[default]
    Session,
    Arrangement,
    /// Piano roll editor focused on a specific MIDI clip. The clip belongs
    /// to a session slot owned by `track_id` × `scene_idx`; we keep both for
    /// preview-on-draw routing (use the track's instrument node id).
    PianoRoll {
        clip_id: ClipId,
        track_id: TrackId,
        scene_idx: usize,
    },
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

    /// Filesystem path the project was last saved to / loaded from.
    /// `None` means it has never been saved (next Ctrl+S triggers Save As).
    project_path: Option<PathBuf>,
    /// True if the project has unsaved changes since the last save / load.
    /// Window title shows `*` while dirty.
    is_dirty: bool,
    /// Wall-clock instant of the last successful save. Drives the toolbar's
    /// "Saved Xs ago" indicator so silent saves (Ctrl+S after a path is set)
    /// give visible feedback.
    last_save_at: Option<Instant>,

    // ----- Piano roll editing state (Slice 8) -----
    /// Selected note index within the piano-roll clip, if any.
    piano_roll_selected: Option<usize>,
    /// In-flight drag state for the piano roll (move / resize).
    piano_roll_drag: DragState,
    /// MIDI notes that the piano roll preview-fired; we owe them a NoteOff
    /// at the corresponding `Instant` to avoid stuck-note voices.
    pending_preview_offs: Vec<(Instant, ondeks_core::NodeId, MidiPitch)>,

    // ----- Slice 9: session-grid playback mirror -----
    /// Mirror of the engine's launcher state, populated from
    /// `RuntimeEvent::SlotStateChanged`. Drives the slot-icon coloring in the
    /// session view; the engine remains the source of truth.
    slot_states: HashMap<(usize, usize), SlotState>,
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
            project_path: None,
            is_dirty: false,
            last_save_at: None,
            piano_roll_selected: None,
            piano_roll_drag: DragState::default(),
            pending_preview_offs: Vec::new(),
            slot_states: HashMap::new(),
        }
    }

    /// Mark the project as having unsaved changes. Called from every
    /// state-mutating path (project dispatcher, mixer actions, undo/redo
    /// replay).
    fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    /// Compose the window title: `* Project — Ondeks` when dirty, no `*` clean.
    fn window_title(&self) -> String {
        let prefix = if self.is_dirty { "* " } else { "" };
        let name = if self.project.meta.name.is_empty() {
            "Untitled"
        } else {
            self.project.meta.name.as_str()
        };
        format!("{prefix}{name} — Ondeks")
    }

    /// Human-readable save status for the toolbar. None when the project has
    /// no path AND has never been saved (caller renders "Unsaved").
    fn save_status_string(&self) -> Option<String> {
        // Strip extension off the filename for compactness.
        let path_label = self.project_path.as_ref().map(|p| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_else(|| {
                    p.to_str().unwrap_or("project")
                })
                .to_string()
        });
        let saved_age = self.last_save_at.map(|t| {
            let secs = t.elapsed().as_secs();
            if secs < 5 {
                "just now".to_string()
            } else if secs < 60 {
                format!("{}s ago", secs)
            } else if secs < 3600 {
                format!("{}m ago", secs / 60)
            } else {
                format!("{}h ago", secs / 3600)
            }
        });

        match (path_label, saved_age, self.is_dirty) {
            (Some(p), Some(age), false) => Some(format!("Saved {age} → {p}")),
            (Some(p), Some(age), true) => Some(format!("Saved {age} → {p} (modified)")),
            (Some(p), None, _) => Some(format!("Loaded → {p}")),
            (None, _, _) => None,
        }
    }

    /// Poll for events from the audio runtime.
    fn poll_runtime_events(&mut self) {
        // Collect first so we can call `&mut self` methods (rebuild view models)
        // inside the per-event branches without borrowing `self.host` twice.
        let events: Vec<_> = self.host.poll_events().collect();
        for event in events {
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
                ondeks_runtime::queue::RuntimeEvent::SlotStateChanged { track, scene, state } => {
                    if matches!(state, SlotState::Empty) {
                        self.slot_states.remove(&(track, scene));
                    } else {
                        self.slot_states.insert((track, scene), state);
                    }
                    self.rebuild_view_models();
                }
                _ => {}
            }
        }
    }

    /// Rebuild project and session view models from the current project.
    fn rebuild_view_models(&mut self) {
        self.project_vm = ProjectViewModel::from_project(&self.project);
        // Snapshot the slot mirror so we don't borrow `self` twice when the
        // closure runs.
        let slot_states = self.slot_states.clone();
        self.session_vm = SessionViewModel::from_project(
            &self.project,
            |track, scene| {
                slot_states
                    .get(&(track, scene))
                    .copied()
                    .unwrap_or(SlotState::Empty)
            },
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
                self.mark_dirty();
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
                self.mark_dirty();
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

            // Save status indicator. Confirms that silent saves
            // (Ctrl+S after a path is set) actually happened.
            ui.separator();
            let save_text = match self.save_status_string() {
                Some(s) => s,
                None => "Unsaved".to_string(),
            };
            let save_color = if self.is_dirty {
                egui::Color32::from_rgb(255, 193, 7)
            } else if self.last_save_at.is_some() {
                egui::Color32::from_rgb(120, 200, 120)
            } else {
                egui::Color32::from_gray(140)
            };
            ui.label(
                egui::RichText::new(save_text)
                    .small()
                    .color(save_color),
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
            CurrentView::PianoRoll { clip_id, track_id, .. } => {
                self.render_piano_roll(ui, clip_id, track_id)
            }
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
        if let Some((track_idx, scene_idx)) = response.slot_double_clicked {
            self.open_or_create_clip_in_slot(track_idx, scene_idx);
        }
        if let Some((track, scene)) = response.slot_play_clicked {
            self.toggle_slot_playback(track, scene);
        }
        if let Some(scene) = response.scene_launched {
            self.launch_scene(scene);
        }
        if let Some(track) = response.track_stopped {
            let _ = self.host.send_command(ondeks_core::Command::StopTrack { track });
        }
        if response.stop_all_clicked {
            let _ = self.host.send_command(ondeks_core::Command::StopAll);
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

    /// Open the piano roll on the clip in (track_idx, scene_idx). If the slot
    /// is empty, create a new MIDI clip first and place it there.
    /// Audio tracks are skipped (they don't have MIDI clips).
    fn open_or_create_clip_in_slot(&mut self, track_idx: usize, scene_idx: usize) {
        let Some(track_vm) = self.session_vm.track_headers.get(track_idx) else {
            return;
        };
        if track_vm.track_type != TrackType::Midi {
            tracing::info!("piano roll only supports MIDI tracks");
            return;
        }
        let track_id = track_vm.id;

        // Find existing clip in slot or create one.
        let existing = self
            .project
            .get_track(track_id)
            .and_then(|t| t.session_slots.get(scene_idx).copied())
            .flatten();
        let clip_id = if let Some(id) = existing {
            id
        } else {
            let midi_clip = MidiClip::new(
                format!("Pattern {}", scene_idx + 1),
                Beats(4.0),
            );
            let id = midi_clip.id();
            self.project.add_clip(Clip::Midi(midi_clip));
            if let Some(t) = self.project.get_track_mut(track_id) {
                t.set_session_slot(scene_idx, Some(id));
            }
            self.mark_dirty();
            self.rebuild_view_models();
            id
        };

        self.current_view = CurrentView::PianoRoll {
            clip_id,
            track_id,
            scene_idx,
        };
        self.piano_roll_selected = None;
        self.piano_roll_drag = DragState::default();
    }

    /// Render the piano roll editor for the clip in `current_view`.
    fn render_piano_roll(
        &mut self,
        ui: &mut egui::Ui,
        clip_id: ClipId,
        track_id: TrackId,
    ) {
        // Snapshot the clip's notes + length so the borrow doesn't conflict
        // with self.dispatch_project_command later.
        let (notes, length) = match self.project.get_clip(clip_id) {
            Some(Clip::Midi(c)) => (c.notes.clone(), c.header.length),
            _ => {
                ui.label("Clip not found.");
                if ui.button("← Back to Session").clicked() {
                    self.current_view = CurrentView::Session;
                }
                return;
            }
        };

        let view = PianoRollView::new(
            &notes,
            length,
            self.piano_roll_selected,
            &mut self.piano_roll_drag,
        );
        let response = view.show(ui);

        if let Some((time, pitch)) = response.add_note {
            let new_note = MidiNote::new(time, Beats(0.125), pitch);
            self.dispatch_project_command(ProjectCommand::AddMidiNote {
                clip_id,
                note: new_note,
            });
            // Select the freshly-added note so subsequent Delete acts on it.
            if let Some(Clip::Midi(c)) = self.project.get_clip(clip_id) {
                self.piano_roll_selected = Some(c.notes.len().saturating_sub(1));
            }
            self.preview_note(track_id, pitch);
        }
        // Pitch previews from clicking the keyboard strip or scrubbing
        // through pitches while dragging a note vertically.
        if let Some(pitch) = response.preview_pitch {
            self.preview_note(track_id, pitch);
        }
        if let Some(sel) = response.set_selected {
            self.piano_roll_selected = sel;
        }
        if let Some((idx, new_time, new_pitch)) = response.commit_move {
            self.dispatch_project_command(ProjectCommand::MoveMidiNote {
                clip_id,
                note_index: idx,
                new_time,
                new_pitch,
            });
        }
        if let Some((idx, new_length)) = response.commit_resize {
            self.dispatch_project_command(ProjectCommand::ResizeMidiNote {
                clip_id,
                note_index: idx,
                new_length,
            });
        }
        if response.delete_selected {
            if let Some(idx) = self.piano_roll_selected {
                self.dispatch_project_command(ProjectCommand::RemoveMidiNote {
                    clip_id,
                    note_index: idx,
                });
                self.piano_roll_selected = None;
            }
        }
        if response.close_clicked {
            self.current_view = CurrentView::Session;
            self.piano_roll_selected = None;
            self.piano_roll_drag = DragState::default();
        }
    }

    /// Slice 9: handle a per-slot play-button click. If the slot is currently
    /// playing or queued, send `StopTrack` (Ableton-style — one slot per
    /// track, so stopping the slot stops the track). Otherwise build a
    /// `ClipPlayback` snapshot and send `LaunchClip`. Auto-starts transport
    /// if it's stopped so launches actually produce sound.
    fn toggle_slot_playback(&mut self, track: usize, scene: usize) {
        let current = self
            .slot_states
            .get(&(track, scene))
            .copied()
            .unwrap_or(SlotState::Empty);
        if matches!(current, SlotState::Playing | SlotState::Queued) {
            let _ = self.host.send_command(ondeks_core::Command::StopTrack { track });
            return;
        }
        let Some(playback) = self.project.clip_playback_for_slot(track, scene) else {
            tracing::info!("Slice 9: no clip in slot ({track}, {scene}); ignoring launch");
            return;
        };
        if !self.transport_vm.is_playing {
            let _ = self.host.send_command(ondeks_core::Command::Play);
        }
        let _ = self.host.send_command(ondeks_core::Command::LaunchClip {
            track,
            scene,
            playback,
        });
    }

    /// Slice 9: launch every non-empty slot in `scene_idx` at the next
    /// quantize boundary. Auto-starts transport if stopped.
    fn launch_scene(&mut self, scene_idx: usize) {
        let playbacks = self.project.scene_playbacks(scene_idx);
        if playbacks.is_empty() {
            return;
        }
        if !self.transport_vm.is_playing {
            let _ = self.host.send_command(ondeks_core::Command::Play);
        }
        let _ = self.host.send_command(ondeks_core::Command::LaunchScene {
            scene: scene_idx,
            playbacks,
        });
    }

    /// Briefly trigger the track's instrument so the user hears the note
    /// they just placed. Note-off is queued via `pending_preview_offs` and
    /// fires from `update()` after a short delay.
    fn preview_note(&mut self, track_id: TrackId, pitch: MidiPitch) {
        let Some(node_id) = self
            .project
            .get_track(track_id)
            .and_then(|t| t.instrument)
        else {
            return;
        };
        let channel = Channel::new(0).expect("channel 0");
        let velocity = Velocity::new(100).expect("velocity 100");
        let _ = self.host.send_command(ondeks_core::Command::SendMidi {
            target: node_id,
            event: MidiEvent::NoteOn {
                channel,
                note: pitch,
                velocity,
            },
            sample_offset: 0,
        });
        self.pending_preview_offs.push((
            Instant::now() + std::time::Duration::from_millis(250),
            node_id,
            pitch,
        ));
    }

    /// Drain `pending_preview_offs` whose deadline has elapsed, sending
    /// NoteOff commands to clear stuck preview voices.
    fn drain_preview_offs(&mut self) {
        if self.pending_preview_offs.is_empty() {
            return;
        }
        let now = Instant::now();
        let channel = Channel::new(0).expect("channel 0");
        let mut still_pending: Vec<_> = Vec::with_capacity(self.pending_preview_offs.len());
        for (deadline, node_id, pitch) in self.pending_preview_offs.drain(..) {
            if now >= deadline {
                let _ = self.host.send_command(ondeks_core::Command::SendMidi {
                    target: node_id,
                    event: MidiEvent::note_off(channel, pitch),
                    sample_offset: 0,
                });
            } else {
                still_pending.push((deadline, node_id, pitch));
            }
        }
        self.pending_preview_offs = still_pending;
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
        self.mark_dirty();
    }

    fn set_master_volume(&mut self, volume_db: f32) {
        self.project.master_mut().volume_db = volume_db;
        let _ = self
            .host
            .send_command(ondeks_core::Command::SetMasterVolume { volume_db });
        self.rebuild_view_models();
        self.mark_dirty();
    }

    /// Replace the current project with a new one. Tears down the engine
    /// graph for the old project's instrument channels and rebuilds it from
    /// the new project (per-track AddInstrumentChannel + mixer state +
    /// master volume). Clears history and saves the path so subsequent saves
    /// don't prompt.
    fn replace_project(&mut self, new_project: Project, path: Option<PathBuf>) {
        // Tear down old graph: emit RemoveInstrumentChannel for every
        // existing MIDI track that has both ids.
        for track in self.project.tracks() {
            if let (Some(synth), Some(strip)) = (track.instrument, track.channel_strip) {
                let _ = self
                    .host
                    .send_command(ondeks_core::Command::RemoveInstrumentChannel {
                        synth_node_id: synth,
                        strip_node_id: strip,
                    });
            }
        }

        // Swap the project before iterating new tracks (we need it owned).
        self.project = new_project;
        self.project_path = path;
        self.is_dirty = false;
        self.last_save_at = None;
        self.history = History::new(100);
        self.rename_buffer = None;

        // Build up new graph.
        for track in self.project.tracks() {
            if let (Some(synth), Some(strip)) = (track.instrument, track.channel_strip) {
                let _ = self
                    .host
                    .send_command(ondeks_core::Command::AddInstrumentChannel {
                        synth_node_id: synth,
                        strip_node_id: strip,
                        track_id: track.id,
                    });
                let _ = self.host.send_command(ondeks_core::Command::SetTrackVolume {
                    node_id: strip,
                    volume_db: track.volume_db,
                });
                let _ = self.host.send_command(ondeks_core::Command::SetTrackPan {
                    node_id: strip,
                    pan: track.pan,
                });
                let _ = self.host.send_command(ondeks_core::Command::SetTrackMute {
                    node_id: strip,
                    muted: track.muted,
                });
                let _ = self.host.send_command(ondeks_core::Command::SetTrackSolo {
                    node_id: strip,
                    soloed: track.soloed,
                });
            }
        }
        // Master volume: read from the master track.
        let master_db = self.project.master().volume_db;
        let _ = self
            .host
            .send_command(ondeks_core::Command::SetMasterVolume {
                volume_db: master_db,
            });

        self.rebuild_view_models();
    }

    /// Save the project to disk. If `path_override` is `Some`, save there and
    /// remember it as the new project_path; otherwise save to the existing
    /// path or open a Save As dialog if there isn't one.
    fn save_project(&mut self, path_override: Option<PathBuf>) {
        let target = path_override.or_else(|| self.project_path.clone()).or_else(|| {
            rfd::FileDialog::new()
                .add_filter("Ondeks Project", &["odk", "json"])
                .set_file_name(format!("{}.odk", self.project.meta.name))
                .save_file()
        });
        let Some(target) = target else {
            return; // user cancelled
        };

        match project_to_json(&self.project) {
            Ok(json) => match std::fs::write(&target, json) {
                Ok(()) => {
                    self.project_path = Some(target.clone());
                    self.is_dirty = false;
                    self.last_save_at = Some(Instant::now());
                    tracing::info!("saved project to {}", target.display());
                }
                Err(e) => tracing::error!("write failed for {}: {e}", target.display()),
            },
            Err(e) => tracing::error!("project_to_json failed: {e}"),
        }
    }

    /// Open a project from disk via a file dialog.
    fn open_project_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Ondeks Project", &["odk", "json"])
            .pick_file()
        else {
            return;
        };
        self.open_project_path(&path);
    }

    /// Open a project file at a known path. Reports failures via tracing
    /// rather than crashing the app.
    fn open_project_path(&mut self, path: &Path) {
        let json = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("read failed for {}: {e}", path.display());
                return;
            }
        };
        match project_from_json(&json) {
            Ok(project) => {
                tracing::info!("loaded project from {}", path.display());
                self.replace_project(project, Some(path.to_path_buf()));
            }
            Err(e) => tracing::error!("parse failed for {}: {e}", path.display()),
        }
    }

    /// Replace the project with a fresh empty one (Ctrl+N).
    fn new_project(&mut self) {
        let project = Project::new("New Project");
        self.replace_project(project, None);
    }

    /// Three-button "What do you want to do?" prompt for destructive actions
    /// when the project has unsaved changes. Returns the user's choice; the
    /// caller dispatches.
    ///
    /// Skipped (returns `Discard`) when the project is clean — common UX
    /// pattern: no friction when there's nothing to save.
    ///
    /// `action_label` is the short verb shown on the action buttons
    /// ("New", "Open", "Quit"). Same UI-thread-blocking gotcha as the file
    /// dialogs — keys held during the dialog don't get their KeyUp delivered,
    /// so callers should reset `keys_down` afterward.
    fn ask_save_choice(&self, action_label: &str) -> SaveChoice {
        if !self.is_dirty {
            return SaveChoice::Discard;
        }
        let project_label = if self.project.meta.name.is_empty() {
            "this project".to_string()
        } else {
            format!("\"{}\"", self.project.meta.name)
        };

        let result = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Info)
            .set_title("What do you want to do?")
            .set_description(&format!(
                "{project_label} has unsaved edits."
            ))
            .set_buttons(rfd::MessageButtons::YesNoCancelCustom(
                format!("Save & {action_label}"),
                format!("Don't save & {action_label}"),
                "Continue working".to_string(),
            ))
            .show();

        // rfd's YesNoCancelCustom returns `Yes`/`No`/`Cancel` on most
        // platforms but on some (e.g. xdg-portal flavors) it returns
        // `Custom(label)`. Handle both shapes.
        match result {
            rfd::MessageDialogResult::Yes => SaveChoice::SaveFirst,
            rfd::MessageDialogResult::No => SaveChoice::Discard,
            rfd::MessageDialogResult::Cancel => SaveChoice::Cancel,
            rfd::MessageDialogResult::Custom(label) => {
                if label.starts_with("Save &") {
                    SaveChoice::SaveFirst
                } else if label.starts_with("Don't save") {
                    SaveChoice::Discard
                } else {
                    SaveChoice::Cancel
                }
            }
            _ => SaveChoice::Cancel,
        }
    }

    /// Run a destructive action with the save-choice prompt in front of it.
    /// `action_label` is what shows on the buttons; `action` is what runs
    /// after Save (if save succeeded) or Discard.
    fn with_save_choice(&mut self, action_label: &str, action: impl FnOnce(&mut Self)) {
        match self.ask_save_choice(action_label) {
            SaveChoice::SaveFirst => {
                self.save_project(None);
                // Save succeeded only if `is_dirty` flipped to false. If the
                // user cancelled the Save As dialog mid-save, dirty stays
                // true and we abort the destructive action.
                if !self.is_dirty {
                    action(self);
                }
            }
            SaveChoice::Discard => action(self),
            SaveChoice::Cancel => {}
        }
    }

    /// Handle Ctrl+S / Ctrl+Shift+S / Ctrl+O / Ctrl+N file shortcuts using
    /// the same explicit event-queue scan as undo/redo (avoids egui's loose
    /// modifier matching swallowing the wrong shortcut).
    fn handle_file_shortcuts(&mut self, ctx: &egui::Context) {
        #[derive(Default)]
        struct Picked {
            save: bool,
            save_as: bool,
            open: bool,
            new: bool,
        }
        let picked = ctx.input_mut(|i| {
            let mut picked = Picked::default();
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
                        match (*key, shift) {
                            (egui::Key::S, false) => {
                                picked.save = true;
                                return false;
                            }
                            (egui::Key::S, true) => {
                                picked.save_as = true;
                                return false;
                            }
                            (egui::Key::O, false) => {
                                picked.open = true;
                                return false;
                            }
                            (egui::Key::N, false) => {
                                picked.new = true;
                                return false;
                            }
                            _ => {}
                        }
                    }
                }
                true
            });
            picked
        });

        let did_action = picked.save || picked.save_as || picked.open || picked.new;

        if picked.save {
            self.save_project(None);
        }
        if picked.save_as {
            // Force re-prompt by clearing project_path before save.
            let prior = self.project_path.take();
            self.save_project(None);
            // If user cancelled, restore prior path so a subsequent Ctrl+S
            // still has a target.
            if self.project_path.is_none() {
                self.project_path = prior;
            }
        }
        if picked.open {
            self.with_save_choice("Open", |app| app.open_project_dialog());
        }
        if picked.new {
            self.with_save_choice("New", |app| app.new_project());
        }

        // Native file dialogs (rfd) block the UI thread on Linux/X11 and
        // Windows. While the dialog is up, egui never sees the KeyUp events
        // for the keys the user was holding (Ctrl, S, etc.) — so on return
        // egui still thinks those keys are pressed. The QWERTY piano keyboard
        // widget reads `key_down` and would keep firing notes; the
        // shortcut handler can also misbehave. Clearing `keys_down` post-
        // action gives us a clean slate; the next real KeyDown re-populates.
        if did_action {
            ctx.input_mut(|i| i.keys_down.clear());
        }
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

/// What the user picked when prompted about unsaved changes before a
/// destructive op (New / Open / Quit). Returned by `ask_save_choice`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SaveChoice {
    /// Save first, then perform the destructive action.
    SaveFirst,
    /// Drop the unsaved edits and perform the destructive action.
    Discard,
    /// Abort the destructive action; keep working in the current project.
    Cancel,
}

impl eframe::App for OndeksApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll for runtime events
        self.poll_runtime_events();

        // Send queued preview NoteOffs whose delay has elapsed (piano roll
        // draws preview-fire the synth and need a NoteOff after ~250ms).
        self.drain_preview_offs();

        // Intercept the OS-level close request (X button, Cmd+Q, etc.) so
        // dirty projects don't get silently lost. egui flips
        // `close_requested` for one frame; we either let it proceed or send
        // CancelClose to keep the window alive.
        if ctx.input(|i| i.viewport().close_requested()) {
            match self.ask_save_choice("Quit") {
                SaveChoice::SaveFirst => {
                    self.save_project(None);
                    // If the user cancelled the Save As dialog mid-save,
                    // dirty stays true → abort the close.
                    if self.is_dirty {
                        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    }
                }
                SaveChoice::Discard => {
                    // Let the close proceed.
                }
                SaveChoice::Cancel => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                }
            }
            // After a confirmation dialog, keys held during the prompt won't
            // get their KeyUp delivered. Reset for consistency with the file
            // shortcut path.
            ctx.input_mut(|i| i.keys_down.clear());
        }

        // File ops first so saving doesn't fight with undo on Ctrl+S timing.
        self.handle_file_shortcuts(ctx);
        // Undo / redo shortcuts
        self.handle_undo_redo_shortcuts(ctx);

        // Window title reflects dirty state and project name.
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.window_title()));

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
