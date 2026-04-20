//! Slice 4: project track CRUD with undo/redo round-trips.
//!
//! These tests exercise the dispatcher/history pair that powers Ctrl+Z /
//! Ctrl+Y on the UI thread. Each test drives a sequence of ProjectCommands
//! through `apply_project_command` and a `History`, then undoes them back and
//! asserts the Project matches its prior state structurally.

use ondeks_core::project::{Project, TrackType};
use ondeks_ui_common::{
    apply_project_command, apply_without_outcome, History, HistoryEntry, UiCommand,
};
use ondeks_ui_common::commands::ProjectCommand;

fn record(history: &mut History, desc: &str, undo: ProjectCommand, redo: ProjectCommand) {
    history.record(HistoryEntry {
        description: desc.to_string(),
        undo: UiCommand::Project(undo),
        redo: UiCommand::Project(redo),
    });
}

fn dispatch(project: &mut Project, history: &mut History, cmd: ProjectCommand) {
    let desc = cmd.description().to_string();
    let outcome = apply_project_command(project, &cmd).expect("dispatch");
    record(history, &desc, outcome.undo, outcome.redo);
}

fn replay(project: &mut Project, cmd: UiCommand) {
    match cmd {
        UiCommand::Project(pc) => {
            apply_without_outcome(project, &pc).expect("replay");
        }
        _ => panic!("unexpected non-project command in history"),
    }
}

fn track_names(project: &Project) -> Vec<String> {
    project.tracks().iter().map(|t| t.name.clone()).collect()
}

#[test]
fn add_undo_redo_preserves_track_identity() {
    let mut project = Project::new("test");
    let mut history = History::new(100);

    dispatch(&mut project, &mut history, ProjectCommand::AddTrack {
        track_type: TrackType::Midi,
        name: "Bass".to_string(),
    });
    assert_eq!(track_names(&project), vec!["Bass", "Master"]);
    let id_after_add = project.tracks()[0].id;

    replay(&mut project, history.undo().unwrap());
    assert_eq!(track_names(&project), vec!["Master"]);

    replay(&mut project, history.redo().unwrap());
    assert_eq!(track_names(&project), vec!["Bass", "Master"]);
    assert_eq!(project.tracks()[0].id, id_after_add, "redo must restore original id");
}

#[test]
fn remove_undo_puts_track_back_at_same_index() {
    let mut project = Project::new("test");
    let mut history = History::new(100);

    dispatch(&mut project, &mut history, ProjectCommand::AddTrack {
        track_type: TrackType::Midi,
        name: "A".to_string(),
    });
    dispatch(&mut project, &mut history, ProjectCommand::AddTrack {
        track_type: TrackType::Midi,
        name: "B".to_string(),
    });
    dispatch(&mut project, &mut history, ProjectCommand::AddTrack {
        track_type: TrackType::Midi,
        name: "C".to_string(),
    });
    // Project layout: [A, B, C, Master]
    assert_eq!(track_names(&project), vec!["A", "B", "C", "Master"]);

    let id_b = project.tracks()[1].id;
    dispatch(&mut project, &mut history, ProjectCommand::RemoveTrack { track_id: id_b });
    assert_eq!(track_names(&project), vec!["A", "C", "Master"]);

    replay(&mut project, history.undo().unwrap());
    assert_eq!(track_names(&project), vec!["A", "B", "C", "Master"]);
}

#[test]
fn rename_undo_restores_prior_name() {
    let mut project = Project::new("test");
    let mut history = History::new(100);

    dispatch(&mut project, &mut history, ProjectCommand::AddTrack {
        track_type: TrackType::Audio,
        name: "Drums".to_string(),
    });
    let id = project.tracks()[0].id;

    dispatch(&mut project, &mut history, ProjectCommand::RenameTrack {
        track_id: id,
        name: "Kit".to_string(),
    });
    assert_eq!(project.get_track(id).unwrap().name, "Kit");

    replay(&mut project, history.undo().unwrap());
    assert_eq!(project.get_track(id).unwrap().name, "Drums");

    replay(&mut project, history.redo().unwrap());
    assert_eq!(project.get_track(id).unwrap().name, "Kit");
}

#[test]
fn move_track_undo_returns_to_original_index() {
    let mut project = Project::new("test");
    let mut history = History::new(100);
    dispatch(&mut project, &mut history, ProjectCommand::AddTrack {
        track_type: TrackType::Midi, name: "A".to_string()
    });
    dispatch(&mut project, &mut history, ProjectCommand::AddTrack {
        track_type: TrackType::Midi, name: "B".to_string()
    });
    dispatch(&mut project, &mut history, ProjectCommand::AddTrack {
        track_type: TrackType::Midi, name: "C".to_string()
    });
    assert_eq!(track_names(&project), vec!["A", "B", "C", "Master"]);

    let id_a = project.tracks()[0].id;
    dispatch(&mut project, &mut history, ProjectCommand::MoveTrack {
        track_id: id_a, new_index: 2
    });
    assert_eq!(project.track_index(id_a), Some(2));

    replay(&mut project, history.undo().unwrap());
    assert_eq!(project.track_index(id_a), Some(0));
}

#[test]
fn cascaded_undo_unwinds_full_session() {
    let mut project = Project::new("test");
    let mut history = History::new(100);

    dispatch(&mut project, &mut history, ProjectCommand::AddTrack {
        track_type: TrackType::Midi, name: "A".to_string()
    });
    dispatch(&mut project, &mut history, ProjectCommand::AddTrack {
        track_type: TrackType::Audio, name: "B".to_string()
    });
    let id_b = project.tracks()[1].id;
    dispatch(&mut project, &mut history, ProjectCommand::RenameTrack {
        track_id: id_b, name: "B'".to_string()
    });
    dispatch(&mut project, &mut history, ProjectCommand::DuplicateTrack {
        track_id: id_b
    });

    // 4 forward commands; undo them all and we end up at the empty project.
    for _ in 0..4 {
        replay(&mut project, history.undo().expect("undo available"));
    }
    assert_eq!(track_names(&project), vec!["Master"]);
    assert!(!history.can_undo());
    assert!(history.can_redo());
}

#[test]
fn duplicate_creates_new_track_not_aliased() {
    let mut project = Project::new("test");
    let mut history = History::new(100);
    dispatch(&mut project, &mut history, ProjectCommand::AddTrack {
        track_type: TrackType::Midi, name: "Pad".to_string()
    });
    let id_orig = project.tracks()[0].id;

    dispatch(&mut project, &mut history, ProjectCommand::DuplicateTrack { track_id: id_orig });
    assert_eq!(track_names(&project), vec!["Pad", "Pad copy", "Master"]);
    let id_dup = project.tracks()[1].id;
    assert_ne!(id_orig, id_dup, "duplicate must have a fresh id");

    // Rename the duplicate; original must be unaffected.
    dispatch(&mut project, &mut history, ProjectCommand::RenameTrack {
        track_id: id_dup, name: "Pad 2".to_string()
    });
    assert_eq!(project.get_track(id_orig).unwrap().name, "Pad");
    assert_eq!(project.get_track(id_dup).unwrap().name, "Pad 2");
}

#[test]
fn cannot_remove_master() {
    let mut project = Project::new("test");
    let master_id = project.master().id;
    let err = apply_project_command(
        &mut project,
        &ProjectCommand::RemoveTrack { track_id: master_id },
    );
    assert!(err.is_err(), "removing master must error");
    assert_eq!(track_names(&project), vec!["Master"]);
}
