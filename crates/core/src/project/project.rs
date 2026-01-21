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

    /// Remove a track.
    pub fn remove_track(&mut self, id: TrackId) -> Result<(), ProjectError> {
        if id == self.master_id {
            return Err(ProjectError::CannotRemoveMaster);
        }

        let pos = self.tracks.iter()
            .position(|t| t.id == id)
            .ok_or(ProjectError::TrackNotFound(id))?;

        self.tracks.remove(pos);
        Ok(())
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
