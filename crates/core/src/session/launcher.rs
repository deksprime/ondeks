use std::collections::HashMap;
use crate::transport::Beats;

/// State of a clip slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SlotState {
    #[default]
    Empty,
    Stopped,
    Queued,
    Playing,
    Recording,
}

/// Quantization for clip launching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LaunchQuantize {
    None,
    #[default]
    Bar,
    Beat,
    HalfBeat,
}

impl LaunchQuantize {
    /// Get next quantized position.
    pub fn next_position(&self, current: Beats, time_sig_numerator: u8) -> Beats {
        match self {
            Self::None => current,
            Self::Bar => {
                let bar_length = time_sig_numerator as f64;
                let next_bar = (current.0 / bar_length).ceil() * bar_length;
                Beats(next_bar)
            }
            Self::Beat => Beats(current.0.ceil()),
            Self::HalfBeat => Beats((current.0 * 2.0).ceil() / 2.0),
        }
    }
}

/// Manages clip launching in session view.
#[derive(Debug)]
pub struct ClipLauncher {
    /// (track_index, scene_index) -> state
    slot_states: HashMap<(usize, usize), SlotState>,
    /// Clips queued to start
    queued_starts: Vec<QueuedAction>,
    /// Clips queued to stop
    queued_stops: Vec<QueuedAction>,
    /// Global launch quantization
    pub quantize: LaunchQuantize,
}

#[derive(Debug, Clone)]
struct QueuedAction {
    track: usize,
    scene: usize,
    trigger_time: Beats,
}

impl ClipLauncher {
    pub fn new() -> Self {
        Self {
            slot_states: HashMap::new(),
            queued_starts: Vec::new(),
            queued_stops: Vec::new(),
            quantize: LaunchQuantize::Bar,
        }
    }

    /// Get slot state.
    pub fn slot_state(&self, track: usize, scene: usize) -> SlotState {
        self.slot_states.get(&(track, scene)).copied().unwrap_or_default()
    }

    /// Set slot state.
    pub fn set_slot_state(&mut self, track: usize, scene: usize, state: SlotState) {
        self.slot_states.insert((track, scene), state);
    }

    /// Launch a clip (queues it).
    pub fn launch_clip(
        &mut self,
        track: usize,
        scene: usize,
        current_pos: Beats,
        time_sig_numerator: u8,
    ) {
        let trigger_time = self.quantize.next_position(current_pos, time_sig_numerator);

        // Stop any playing clip on this track
        for (&(t, _s), state) in &mut self.slot_states {
            if t == track && *state == SlotState::Playing {
                *state = SlotState::Stopped;
            }
        }

        // Queue the new clip
        self.set_slot_state(track, scene, SlotState::Queued);
        self.queued_starts.push(QueuedAction {
            track,
            scene,
            trigger_time,
        });
    }

    /// Launch all clips in a scene.
    pub fn launch_scene(
        &mut self,
        scene: usize,
        num_tracks: usize,
        current_pos: Beats,
        time_sig_numerator: u8,
    ) {
        for track in 0..num_tracks {
            if self.slot_state(track, scene) != SlotState::Empty {
                self.launch_clip(track, scene, current_pos, time_sig_numerator);
            }
        }
    }

    /// Stop clip on a track.
    pub fn stop_track(&mut self, track: usize) {
        for (&(t, _s), state) in &mut self.slot_states {
            if t == track && (*state == SlotState::Playing || *state == SlotState::Queued) {
                *state = SlotState::Stopped;
            }
        }
    }

    /// Stop all clips.
    pub fn stop_all(&mut self) {
        for state in self.slot_states.values_mut() {
            if *state == SlotState::Playing || *state == SlotState::Queued {
                *state = SlotState::Stopped;
            }
        }
        self.queued_starts.clear();
        self.queued_stops.clear();
    }

    /// Process queued actions at the given position.
    pub fn process(&mut self, current_pos: Beats) -> Vec<LaunchEvent> {
        let mut events = Vec::new();

        // Process queued starts
        self.queued_starts.retain(|action| {
            if current_pos.0 >= action.trigger_time.0 {
                self.slot_states.insert((action.track, action.scene), SlotState::Playing);
                events.push(LaunchEvent::ClipStarted {
                    track: action.track,
                    scene: action.scene,
                });
                false
            } else {
                true
            }
        });

        events
    }
}

/// Events emitted by the launcher.
#[derive(Debug, Clone)]
pub enum LaunchEvent {
    ClipStarted { track: usize, scene: usize },
    ClipStopped { track: usize, scene: usize },
}

impl Default for ClipLauncher {
    fn default() -> Self {
        Self::new()
    }
}
