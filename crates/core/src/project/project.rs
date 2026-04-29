use std::collections::HashMap;
use crate::ids::{TrackId, ClipId, SceneId};
use crate::transport::{TempoMap, TimeSignature, Beats};
use crate::error::ProjectError;
use super::track::{Track, TrackType};
use super::clip::Clip;

/// Project metadata.
#[derive(Debug, Clone)]
pub struct ProjectMeta {
    pub name: String,
    pub author: String,
    pub created: String,
    pub modified: String,
}

impl Default for ProjectMeta {
    fn default() -> Self {
        Self {
            name: "Untitled".to_string(),
            author: String::new(),
            created: String::new(),
            modified: String::new(),
        }
    }
}

/// A scene in session view.
#[derive(Debug, Clone)]
pub struct Scene {
    pub id: SceneId,
    pub name: String,
    pub tempo: Option<f64>,
    pub time_signature: Option<TimeSignature>,
}

impl Scene {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: SceneId::generate(),
            name: name.into(),
            tempo: None,
            time_signature: None,
        }
    }
}

/// The main project structure.
#[derive(Debug)]
pub struct Project {
    pub meta: ProjectMeta,
    tracks: Vec<Track>,
    clips: HashMap<ClipId, Clip>,
    scenes: Vec<Scene>,
    pub tempo_map: TempoMap,
    pub time_signature: TimeSignature,
    master_id: TrackId,
}

impl Project {
    /// Create a new empty project.
    pub fn new(name: impl Into<String>) -> Self {
        let master = Track::master();
        let master_id = master.id;

        Self {
            meta: ProjectMeta {
                name: name.into(),
                ..Default::default()
            },
            tracks: vec![master],
            clips: HashMap::new(),
            scenes: vec![Scene::new("Scene 1")],
            tempo_map: TempoMap::default(),
            time_signature: TimeSignature::FOUR_FOUR,
            master_id,
        }
    }

    // --- Track Management ---

    /// Add a new track.
    pub fn add_track(&mut self, track_type: TrackType, name: impl Into<String>) -> TrackId {
        let track = Track::new(track_type, name);
        let id = track.id;
        // Insert before master
        let master_pos = self.tracks.iter().position(|t| t.id == self.master_id).unwrap();
        self.tracks.insert(master_pos, track);
        id
    }

    /// Remove a track, returning the removed track and its index for undo.
    pub fn remove_track(&mut self, id: TrackId) -> Result<(Track, usize), ProjectError> {
        if id == self.master_id {
            return Err(ProjectError::CannotRemoveMaster);
        }

        let pos = self.tracks.iter()
            .position(|t| t.id == id)
            .ok_or(ProjectError::TrackNotFound(id))?;

        let track = self.tracks.remove(pos);
        Ok((track, pos))
    }

    /// Insert a pre-constructed track at a specific index, shifting existing tracks.
    ///
    /// Used for undo of [`remove_track`] and for undo of [`move_track`]. Clamps
    /// `insert_at` so master stays last. Returns an error if master is inserted
    /// or if a duplicate ID already exists.
    pub fn insert_track_at(&mut self, track: Track, insert_at: usize) -> Result<(), ProjectError> {
        if track.track_type == TrackType::Master {
            return Err(ProjectError::CannotRemoveMaster);
        }
        if self.tracks.iter().any(|t| t.id == track.id) {
            return Err(ProjectError::TrackNotFound(track.id));
        }
        let master_pos = self.tracks.iter().position(|t| t.id == self.master_id).unwrap();
        let clamped = insert_at.min(master_pos);
        self.tracks.insert(clamped, track);
        Ok(())
    }

    /// Move a track to a new index (excluding master). Clamps so master stays last.
    pub fn move_track(&mut self, id: TrackId, new_index: usize) -> Result<usize, ProjectError> {
        if id == self.master_id {
            return Err(ProjectError::CannotRemoveMaster);
        }
        let old_pos = self.tracks.iter()
            .position(|t| t.id == id)
            .ok_or(ProjectError::TrackNotFound(id))?;
        let master_pos = self.tracks.iter().position(|t| t.id == self.master_id).unwrap();
        // Upper bound is master_pos - 1 since master must remain last.
        let max_index = master_pos.saturating_sub(if old_pos < master_pos { 1 } else { 0 });
        let clamped = new_index.min(max_index);
        if clamped == old_pos {
            return Ok(old_pos);
        }
        let track = self.tracks.remove(old_pos);
        let adjusted = if clamped > old_pos { clamped } else { clamped };
        self.tracks.insert(adjusted, track);
        Ok(old_pos)
    }

    /// Index of a track in the track list, or None if not found.
    pub fn track_index(&self, id: TrackId) -> Option<usize> {
        self.tracks.iter().position(|t| t.id == id)
    }

    /// Replace the entire scene list (used by the persistence loader).
    /// Master scene "Scene 1" is appended automatically if `scenes` is empty
    /// so the project always has at least one row.
    pub fn replace_scenes(&mut self, mut scenes: Vec<Scene>) {
        if scenes.is_empty() {
            scenes.push(Scene::new("Scene 1"));
        }
        self.scenes = scenes;
    }

    /// Replace the entire track list (used by the persistence loader).
    ///
    /// Picks up the master track from the loaded list (must be present) and
    /// updates `master_id` so master accessors keep working. Errors if no
    /// master is present or if multiple masters are.
    pub fn replace_tracks(&mut self, tracks: Vec<Track>) -> Result<(), ProjectError> {
        let master_count = tracks
            .iter()
            .filter(|t| t.track_type == TrackType::Master)
            .count();
        if master_count != 1 {
            return Err(ProjectError::CannotRemoveMaster);
        }
        let master = tracks
            .iter()
            .find(|t| t.track_type == TrackType::Master)
            .expect("master track present");
        self.master_id = master.id;
        self.tracks = tracks;
        Ok(())
    }

    /// Arm a single track exclusively, disarming all others. Returns the
    /// previously-armed track id (if any) so callers that care about undo can
    /// remember it; arm is intentionally non-undoable though — it's an
    /// ephemeral routing flag, like transport Play/Stop.
    pub fn arm_exclusive(&mut self, id: TrackId) -> Option<TrackId> {
        let mut previously_armed: Option<TrackId> = None;
        for track in self.tracks.iter_mut() {
            if track.armed {
                previously_armed = Some(track.id);
            }
            track.armed = track.id == id;
        }
        previously_armed
    }

    /// Disarm all tracks.
    pub fn disarm_all(&mut self) {
        for track in self.tracks.iter_mut() {
            track.armed = false;
        }
    }

    /// The currently-armed track, if any. Returns the first armed track if
    /// multiple somehow end up armed (shouldn't happen via `arm_exclusive`).
    pub fn armed_track(&self) -> Option<&Track> {
        self.tracks.iter().find(|t| t.armed)
    }

    /// Get a track by ID.
    pub fn get_track(&self, id: TrackId) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    /// Get a mutable track by ID.
    pub fn get_track_mut(&mut self, id: TrackId) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    /// Get all tracks.
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    /// Get master track.
    pub fn master(&self) -> &Track {
        self.get_track(self.master_id).unwrap()
    }

    /// Get master track mutably.
    pub fn master_mut(&mut self) -> &mut Track {
        self.get_track_mut(self.master_id).unwrap()
    }

    // --- Clip Management ---

    /// Add a clip to the project.
    pub fn add_clip(&mut self, clip: Clip) -> ClipId {
        let id = clip.id();
        self.clips.insert(id, clip);
        id
    }

    /// Remove a clip.
    pub fn remove_clip(&mut self, id: ClipId) -> Result<Clip, ProjectError> {
        self.clips.remove(&id).ok_or(ProjectError::ClipNotFound(id))
    }

    /// Get a clip by ID.
    pub fn get_clip(&self, id: ClipId) -> Option<&Clip> {
        self.clips.get(&id)
    }

    /// Get a mutable clip by ID.
    pub fn get_clip_mut(&mut self, id: ClipId) -> Option<&mut Clip> {
        self.clips.get_mut(&id)
    }

    /// Get all clips.
    pub fn clips(&self) -> impl Iterator<Item = &Clip> {
        self.clips.values()
    }

    /// Place a clip on a track's arrangement.
    pub fn place_clip(
        &mut self,
        track_id: TrackId,
        clip_id: ClipId,
        position: Beats,
    ) -> Result<(), ProjectError> {
        let clip = self.clips.get(&clip_id)
            .ok_or(ProjectError::ClipNotFound(clip_id))?;
        let length = clip.length();

        let track = self.get_track_mut(track_id)
            .ok_or(ProjectError::TrackNotFound(track_id))?;

        track.place_clip(clip_id, position, length);
        Ok(())
    }

    // --- Scene Management ---

    /// Add a scene.
    pub fn add_scene(&mut self, name: impl Into<String>) -> SceneId {
        let scene = Scene::new(name);
        let id = scene.id;
        self.scenes.push(scene);
        id
    }

    /// Get scenes.
    pub fn scenes(&self) -> &[Scene] {
        &self.scenes
    }

    /// Get scene count.
    pub fn scene_count(&self) -> usize {
        self.scenes.len()
    }
}

impl Default for Project {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::clip::MidiClip;

    #[test]
    fn new_project_has_master() {
        let project = Project::new("Test");
        assert_eq!(project.tracks().len(), 1);
        assert_eq!(project.master().track_type, TrackType::Master);
    }

    #[test]
    fn add_and_remove_track() {
        let mut project = Project::new("Test");
        let id = project.add_track(TrackType::Midi, "Bass");

        assert_eq!(project.tracks().len(), 2);
        assert!(project.get_track(id).is_some());

        project.remove_track(id).unwrap();
        assert_eq!(project.tracks().len(), 1);
    }

    #[test]
    fn cannot_remove_master() {
        let mut project = Project::new("Test");
        let master_id = project.master().id;
        assert!(matches!(
            project.remove_track(master_id),
            Err(ProjectError::CannotRemoveMaster)
        ));
    }

    #[test]
    fn add_clip_and_place() {
        let mut project = Project::new("Test");
        let track_id = project.add_track(TrackType::Midi, "Lead");

        let clip = MidiClip::new("Pattern 1", Beats(4.0));
        let clip_id = project.add_clip(Clip::Midi(clip));

        project.place_clip(track_id, clip_id, Beats(0.0)).unwrap();

        let track = project.get_track(track_id).unwrap();
        assert_eq!(track.arrangement_clips.len(), 1);
        assert_eq!(track.arrangement_clips[0].clip_id, clip_id);
    }
}
