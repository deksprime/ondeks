//! Project command dispatcher with undo/redo support.
//!
//! Forward commands flow in, the dispatcher mutates `Project` and returns an
//! [`ApplyOutcome`] that the caller uses to record a `HistoryEntry`. Inverse
//! commands (`undo`) can be fed back into [`apply_project_command`] to restore
//! the previous state; they return their own inverse so redo works.
//!
//! As of Slice 5 the outcome also carries **engine commands** — audio-thread
//! mutations that need to run alongside the project mutation (e.g. creating
//! the graph node for a new MIDI track's instrument). The UI layer is
//! responsible for forwarding these to the `Host`; the dispatcher itself is
//! audio-thread-agnostic.
//!
//! The dispatcher is pure: given a `Project` and a command it mutates and
//! emits an outcome, no hidden global state.

use ondeks_core::Command as EngineCommand;
use ondeks_core::ProjectError;
use ondeks_core::project::{Project, Track, TrackType};

use crate::commands::ProjectCommand;

/// Result of applying a [`ProjectCommand`].
#[derive(Debug, Clone)]
pub struct ApplyOutcome {
    /// The command that reverses this mutation. Pushed into the undo stack.
    pub undo: ProjectCommand,
    /// The command that reproduces this mutation's post-state — used as the
    /// redo when the user Ctrl+Y's back into it. For commands with new-id
    /// creation (Add/Duplicate) this is a `RestoreTrack` carrying the exact
    /// new track snapshot; for other commands it's just the original command
    /// (applying it again is deterministic).
    pub redo: ProjectCommand,
    /// Engine commands to forward to the audio thread. Typically graph
    /// mutations (`AddSynthNode`, `RemoveSynthNode`) that track project
    /// mutations. Empty for commands that don't affect the graph.
    pub engine_commands: Vec<EngineCommand>,
}

impl ApplyOutcome {
    fn project_only(undo: ProjectCommand, redo: ProjectCommand) -> Self {
        Self { undo, redo, engine_commands: Vec::new() }
    }
}

/// Apply a project command to `project`, returning the inverse + redo
/// commands and any engine commands that should mirror the mutation. If the
/// command is itself an inverse produced by a previous call (e.g.
/// `RestoreTrack`), the returned `undo` flips it back.
///
/// This function is the single dispatch point for all project mutations that
/// participate in undo/redo.
pub fn apply_project_command(
    project: &mut Project,
    cmd: &ProjectCommand,
) -> Result<ApplyOutcome, ProjectError> {
    match cmd {
        ProjectCommand::AddTrack { track_type, name } => {
            let new_id = project.add_track(*track_type, name.clone());
            let track = project
                .get_track(new_id)
                .expect("freshly added track must exist")
                .clone();
            let insert_at = project.track_index(new_id).expect("just inserted");
            let engine_commands = engine_commands_for_track(&track, true);
            Ok(ApplyOutcome {
                undo: ProjectCommand::RemoveTrack { track_id: new_id },
                redo: ProjectCommand::RestoreTrack {
                    track: Box::new(track),
                    insert_at,
                },
                engine_commands,
            })
        }

        ProjectCommand::RemoveTrack { track_id } => {
            let (track, index) = project.remove_track(*track_id)?;
            let engine_commands = engine_commands_for_track(&track, false);
            Ok(ApplyOutcome {
                undo: ProjectCommand::RestoreTrack {
                    track: Box::new(track),
                    insert_at: index,
                },
                redo: ProjectCommand::RemoveTrack { track_id: *track_id },
                engine_commands,
            })
        }

        ProjectCommand::RestoreTrack { track, insert_at } => {
            let restored_id = track.id;
            let engine_commands = engine_commands_for_track(track, true);
            project.insert_track_at((**track).clone(), *insert_at)?;
            Ok(ApplyOutcome {
                undo: ProjectCommand::RemoveTrack { track_id: restored_id },
                redo: ProjectCommand::RestoreTrack {
                    track: track.clone(),
                    insert_at: *insert_at,
                },
                engine_commands,
            })
        }

        ProjectCommand::RenameTrack { track_id, name } => {
            let track = project
                .get_track_mut(*track_id)
                .ok_or(ProjectError::TrackNotFound(*track_id))?;
            let old_name = std::mem::replace(&mut track.name, name.clone());
            Ok(ApplyOutcome::project_only(
                ProjectCommand::RenameTrack {
                    track_id: *track_id,
                    name: old_name,
                },
                ProjectCommand::RenameTrack {
                    track_id: *track_id,
                    name: name.clone(),
                },
            ))
        }

        ProjectCommand::SetTrackColor { track_id, color } => {
            let track = project
                .get_track_mut(*track_id)
                .ok_or(ProjectError::TrackNotFound(*track_id))?;
            let old_color = track.color;
            track.color = *color;
            Ok(ApplyOutcome::project_only(
                ProjectCommand::SetTrackColor {
                    track_id: *track_id,
                    color: old_color,
                },
                ProjectCommand::SetTrackColor {
                    track_id: *track_id,
                    color: *color,
                },
            ))
        }

        ProjectCommand::MoveTrack { track_id, new_index } => {
            let old_index = project.move_track(*track_id, *new_index)?;
            Ok(ApplyOutcome::project_only(
                ProjectCommand::MoveTrack {
                    track_id: *track_id,
                    new_index: old_index,
                },
                ProjectCommand::MoveTrack {
                    track_id: *track_id,
                    new_index: *new_index,
                },
            ))
        }

        ProjectCommand::DuplicateTrack { track_id } => {
            let source = project
                .get_track(*track_id)
                .ok_or(ProjectError::TrackNotFound(*track_id))?;
            let mut clone = source.clone();
            clone.id = ondeks_core::TrackId::generate();
            // Fresh node ids for MIDI tracks — the duplicate gets its own
            // instrument + strip pair.
            if clone.track_type == TrackType::Midi {
                clone.instrument = Some(ondeks_core::NodeId::generate());
                clone.channel_strip = Some(ondeks_core::NodeId::generate());
            }
            clone.name = format!("{} copy", clone.name);
            let source_index = project.track_index(*track_id).unwrap();
            let insert_at = source_index + 1;
            let clone_for_outcome = clone.clone();
            let new_id = clone.id;
            let engine_commands = engine_commands_for_track(&clone, true);
            project.insert_track_at(clone, insert_at)?;
            Ok(ApplyOutcome {
                undo: ProjectCommand::RemoveTrack { track_id: new_id },
                redo: ProjectCommand::RestoreTrack {
                    track: Box::new(clone_for_outcome),
                    insert_at,
                },
                engine_commands,
            })
        }

        ProjectCommand::ArmTrack { track_id } => {
            // Non-undoable: arm is ephemeral. We still return an outcome so
            // the UI layer can treat the dispatcher uniformly — but the
            // outcome carries no meaningful undo/redo and empty engine cmds.
            // The UI layer skips history.record() for ArmTrack.
            if project.get_track(*track_id).is_none() {
                return Err(ProjectError::TrackNotFound(*track_id));
            }
            project.arm_exclusive(*track_id);
            Ok(ApplyOutcome::project_only(
                // Echo back the same command as both undo and redo so that if
                // something does push this into history, replaying is still
                // safe (idempotent).
                ProjectCommand::ArmTrack { track_id: *track_id },
                ProjectCommand::ArmTrack { track_id: *track_id },
            ))
        }

        // Unimplemented variants: Scene CRUD and file ops are deferred to
        // later slices (Slice 7 covers Save/Load, Slice 9 adds scene CRUD).
        ProjectCommand::New { .. }
        | ProjectCommand::Save { .. }
        | ProjectCommand::Load { .. }
        | ProjectCommand::AddScene { .. }
        | ProjectCommand::RemoveScene { .. }
        | ProjectCommand::RenameScene { .. } => Err(ProjectError::Unsupported(
            cmd.description().to_string(),
        )),
    }
}

/// Convenience: run the command but don't compute an outcome. Used when
/// replaying inverse commands during undo/redo (history already has the
/// redo/undo pair; we just need to mutate state).
pub fn apply_without_outcome(
    project: &mut Project,
    cmd: &ProjectCommand,
) -> Result<(), ProjectError> {
    apply_project_command(project, cmd).map(|_| ())
}

/// Produce engine commands appropriate for a track that is being added
/// (`adding = true`) or removed (`adding = false`). MIDI tracks with a
/// pre-assigned `instrument` + `channel_strip` pair drive
/// `AddInstrumentChannel` / `RemoveInstrumentChannel`; other track types
/// have no graph representation yet.
fn engine_commands_for_track(track: &Track, adding: bool) -> Vec<EngineCommand> {
    let (Some(synth_id), Some(strip_id)) = (track.instrument, track.channel_strip) else {
        return Vec::new();
    };
    if adding {
        vec![EngineCommand::AddInstrumentChannel {
            synth_node_id: synth_id,
            strip_node_id: strip_id,
            track_id: track.id,
        }]
    } else {
        vec![EngineCommand::RemoveInstrumentChannel {
            synth_node_id: synth_id,
            strip_node_id: strip_id,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ondeks_core::Color;
    use ondeks_core::project::TrackType;

    fn add(project: &mut Project, name: &str) -> ApplyOutcome {
        apply_project_command(
            project,
            &ProjectCommand::AddTrack {
                track_type: TrackType::Midi,
                name: name.to_string(),
            },
        )
        .expect("add succeeds")
    }

    #[test]
    fn add_then_undo_restores_empty() {
        let mut p = Project::new("t");
        assert_eq!(p.tracks().len(), 1);
        let outcome = add(&mut p, "Bass");
        assert_eq!(p.tracks().len(), 2);

        apply_project_command(&mut p, &outcome.undo).unwrap();
        assert_eq!(p.tracks().len(), 1);
    }

    #[test]
    fn add_undo_redo_restores_exact_state() {
        let mut p = Project::new("t");
        let outcome = add(&mut p, "Bass");
        let before_id = p.tracks()[0].id;
        let before_instrument = p.tracks()[0].instrument;

        apply_project_command(&mut p, &outcome.undo).unwrap();
        apply_project_command(&mut p, &outcome.redo).unwrap();

        assert_eq!(p.tracks().len(), 2);
        assert_eq!(p.tracks()[0].id, before_id);
        assert_eq!(
            p.tracks()[0].instrument,
            before_instrument,
            "redo must preserve instrument node id"
        );
    }

    #[test]
    fn add_midi_track_emits_add_instrument_channel() {
        let mut p = Project::new("t");
        let outcome = add(&mut p, "Bass");
        let synth_id = p.tracks()[0].instrument.expect("midi track has instrument");
        let strip_id = p.tracks()[0].channel_strip.expect("midi track has strip");

        assert_eq!(outcome.engine_commands.len(), 1);
        match &outcome.engine_commands[0] {
            EngineCommand::AddInstrumentChannel { synth_node_id, strip_node_id, .. } => {
                assert_eq!(*synth_node_id, synth_id);
                assert_eq!(*strip_node_id, strip_id);
            }
            c => panic!("expected AddInstrumentChannel, got {c:?}"),
        }
    }

    #[test]
    fn add_audio_track_emits_no_engine_command() {
        let mut p = Project::new("t");
        let outcome = apply_project_command(
            &mut p,
            &ProjectCommand::AddTrack {
                track_type: TrackType::Audio,
                name: "Drums".to_string(),
            },
        )
        .unwrap();
        assert!(outcome.engine_commands.is_empty());
        assert!(p.tracks()[0].instrument.is_none());
        assert!(p.tracks()[0].channel_strip.is_none());
    }

    #[test]
    fn remove_midi_track_emits_remove_instrument_channel() {
        let mut p = Project::new("t");
        let add_out = add(&mut p, "Bass");
        let id = if let ProjectCommand::RemoveTrack { track_id } = add_out.undo {
            track_id
        } else {
            unreachable!();
        };
        let synth_id = p.get_track(id).unwrap().instrument.unwrap();
        let strip_id = p.get_track(id).unwrap().channel_strip.unwrap();

        let rm = apply_project_command(
            &mut p,
            &ProjectCommand::RemoveTrack { track_id: id },
        )
        .unwrap();
        assert_eq!(rm.engine_commands.len(), 1);
        match &rm.engine_commands[0] {
            EngineCommand::RemoveInstrumentChannel { synth_node_id, strip_node_id } => {
                assert_eq!(*synth_node_id, synth_id);
                assert_eq!(*strip_node_id, strip_id);
            }
            c => panic!("expected RemoveInstrumentChannel, got {c:?}"),
        }
    }

    #[test]
    fn restore_after_remove_reuses_same_node_ids() {
        let mut p = Project::new("t");
        let _ = add(&mut p, "Bass");
        let id = p.tracks()[0].id;
        let synth_id = p.tracks()[0].instrument.unwrap();
        let strip_id = p.tracks()[0].channel_strip.unwrap();

        let rm = apply_project_command(&mut p, &ProjectCommand::RemoveTrack { track_id: id }).unwrap();
        let restore_out = apply_project_command(&mut p, &rm.undo).unwrap();

        assert_eq!(restore_out.engine_commands.len(), 1);
        match &restore_out.engine_commands[0] {
            EngineCommand::AddInstrumentChannel { synth_node_id, strip_node_id, .. } => {
                assert_eq!(*synth_node_id, synth_id);
                assert_eq!(*strip_node_id, strip_id);
            }
            c => panic!("expected AddInstrumentChannel, got {c:?}"),
        }
        assert_eq!(p.tracks()[0].instrument, Some(synth_id));
        assert_eq!(p.tracks()[0].channel_strip, Some(strip_id));
    }

    #[test]
    fn rename_undo_restores_old_name() {
        let mut p = Project::new("t");
        let add_out = add(&mut p, "Bass");
        let id = if let ProjectCommand::RemoveTrack { track_id } = add_out.undo {
            track_id
        } else {
            panic!("expected RemoveTrack undo");
        };

        let rename_out = apply_project_command(
            &mut p,
            &ProjectCommand::RenameTrack {
                track_id: id,
                name: "Sub".to_string(),
            },
        )
        .unwrap();
        assert_eq!(p.get_track(id).unwrap().name, "Sub");
        assert!(rename_out.engine_commands.is_empty());

        apply_project_command(&mut p, &rename_out.undo).unwrap();
        assert_eq!(p.get_track(id).unwrap().name, "Bass");
    }

    #[test]
    fn remove_undo_restores_full_track_at_original_index() {
        let mut p = Project::new("t");
        let out_a = add(&mut p, "A");
        let _ = add(&mut p, "B");
        let _ = add(&mut p, "C");

        let id_a = if let ProjectCommand::RemoveTrack { track_id } = out_a.undo {
            track_id
        } else {
            unreachable!()
        };

        let rm = apply_project_command(&mut p, &ProjectCommand::RemoveTrack { track_id: id_a })
            .unwrap();
        assert!(p.get_track(id_a).is_none());

        apply_project_command(&mut p, &rm.undo).unwrap();
        assert_eq!(p.track_index(id_a), Some(0), "A should return to slot 0");
    }

    #[test]
    fn move_undo_returns_original_index() {
        let mut p = Project::new("t");
        let a = add(&mut p, "A");
        let _ = add(&mut p, "B");
        let _ = add(&mut p, "C");
        let id_a = if let ProjectCommand::RemoveTrack { track_id } = a.undo {
            track_id
        } else {
            unreachable!()
        };

        assert_eq!(p.track_index(id_a), Some(0));
        let mv = apply_project_command(
            &mut p,
            &ProjectCommand::MoveTrack {
                track_id: id_a,
                new_index: 2,
            },
        )
        .unwrap();
        assert_eq!(p.track_index(id_a), Some(2));

        apply_project_command(&mut p, &mv.undo).unwrap();
        assert_eq!(p.track_index(id_a), Some(0));
    }

    #[test]
    fn set_color_undo_returns_prior_color() {
        let mut p = Project::new("t");
        let a = add(&mut p, "A");
        let id = if let ProjectCommand::RemoveTrack { track_id } = a.undo {
            track_id
        } else {
            unreachable!()
        };

        let orig = p.get_track(id).unwrap().color;
        let target = Color::RED;

        let sc = apply_project_command(
            &mut p,
            &ProjectCommand::SetTrackColor {
                track_id: id,
                color: target,
            },
        )
        .unwrap();
        assert_eq!(p.get_track(id).unwrap().color, target);

        apply_project_command(&mut p, &sc.undo).unwrap();
        assert_eq!(p.get_track(id).unwrap().color, orig);
    }

    #[test]
    fn duplicate_creates_new_ids_and_fresh_node_pair() {
        let mut p = Project::new("t");
        let a = add(&mut p, "A");
        let id = if let ProjectCommand::RemoveTrack { track_id } = a.undo {
            track_id
        } else {
            unreachable!()
        };
        let original_instrument = p.get_track(id).unwrap().instrument;
        let original_strip = p.get_track(id).unwrap().channel_strip;

        let dup = apply_project_command(&mut p, &ProjectCommand::DuplicateTrack { track_id: id })
            .unwrap();
        assert_eq!(p.tracks().len(), 3);
        let new_id = if let ProjectCommand::RemoveTrack { track_id } = dup.undo {
            track_id
        } else {
            unreachable!()
        };
        assert_ne!(new_id, id);
        assert_eq!(p.get_track(new_id).unwrap().name, "A copy");

        let new_instrument = p.get_track(new_id).unwrap().instrument;
        let new_strip = p.get_track(new_id).unwrap().channel_strip;
        assert_ne!(new_instrument, original_instrument);
        assert_ne!(new_strip, original_strip);

        apply_project_command(&mut p, &dup.undo).unwrap();
        assert!(p.get_track(new_id).is_none());
        assert_eq!(p.tracks().len(), 2);
    }

    #[test]
    fn arm_exclusive_disarms_others() {
        let mut p = Project::new("t");
        let _ = add(&mut p, "A");
        let _ = add(&mut p, "B");
        let id_a = p.tracks()[0].id;
        let id_b = p.tracks()[1].id;

        apply_project_command(&mut p, &ProjectCommand::ArmTrack { track_id: id_a }).unwrap();
        assert!(p.get_track(id_a).unwrap().armed);
        assert!(!p.get_track(id_b).unwrap().armed);

        apply_project_command(&mut p, &ProjectCommand::ArmTrack { track_id: id_b }).unwrap();
        assert!(!p.get_track(id_a).unwrap().armed);
        assert!(p.get_track(id_b).unwrap().armed);

        assert_eq!(p.armed_track().map(|t| t.id), Some(id_b));
    }
}
