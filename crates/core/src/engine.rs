//! Audio engine: owns the graph, processor, and transport. Exposes a single
//! `process()` entry point that the runtime's audio callback invokes each block.

use crate::command::Command;
use crate::dsp::StereoBuffer;
use crate::graph::nodes::{ChannelStripNode, MasterStripNode, SynthNode};
use crate::graph::{AudioGraph, AudioNode, GraphProcessor, PortAddress, ProcessContext};
use crate::ids::NodeId;
use crate::session::{ClipLauncher, EngineMidiDispatch, SlotStateChange};
use crate::transport::{Beats, Transport};

/// The audio engine. One instance lives on the audio thread.
pub struct Engine {
    graph: AudioGraph,
    processor: GraphProcessor,
    transport: Transport,
    sample_rate: u32,
    /// Master strip node. Sits between all per-track channel strips and the
    /// `OutputNode`; its gain is the master fader.
    master_strip_id: NodeId,
    /// Session clip launcher. Owns slot states + active clip playback. Driven
    /// every block by [`Self::process`].
    clip_launcher: ClipLauncher,
    /// Slot-state transitions waiting to be drained by the host into
    /// `RuntimeEvent::SlotStateChanged`.
    pending_state_changes: Vec<SlotStateChange>,
}

impl Engine {
    /// Create a new engine with a default graph: `MasterStripNode → OutputNode`.
    /// Per-track instrument channels are added on demand via
    /// `Command::AddInstrumentChannel` as the UI creates MIDI tracks.
    pub fn new(sample_rate: u32, buffer_size: usize) -> Self {
        let mut graph = AudioGraph::new();

        // Master strip wired into the output node's stereo inputs.
        let master_strip = MasterStripNode::with_id(NodeId::generate(), sample_rate);
        let master_strip_id = master_strip.id();
        let strip_outs = master_strip
            .outputs()
            .iter()
            .map(|p| p.id)
            .collect::<Vec<_>>();
        graph.add_node_with_id(master_strip_id, Box::new(master_strip));

        let output_id = graph.output();
        let (out_left, out_right) = {
            let output_node = graph
                .get_node(output_id)
                .expect("output node must exist after graph construction");
            let inputs = output_node.inputs();
            (inputs[0].id, inputs[1].id)
        };

        graph
            .connect(
                PortAddress::new(master_strip_id, strip_outs[0]),
                PortAddress::new(output_id, out_left),
            )
            .expect("master L → output L");
        graph
            .connect(
                PortAddress::new(master_strip_id, strip_outs[1]),
                PortAddress::new(output_id, out_right),
            )
            .expect("master R → output R");

        Self {
            graph,
            processor: GraphProcessor::new(buffer_size),
            transport: Transport::new(sample_rate),
            sample_rate,
            master_strip_id,
            clip_launcher: ClipLauncher::new(),
            pending_state_changes: Vec::new(),
        }
    }

    /// Process one block of audio.
    pub fn process(&mut self, output: &mut StereoBuffer, frames: u32) {
        // Advance the clip launcher across this block, but only when the
        // transport is rolling — otherwise queued clips would dequeue the
        // moment they were enqueued (block_end == block_start with the
        // transport stopped, which the launcher treats as "trigger time has
        // passed"). Stopped transport ⇒ launcher idle.
        if self.transport.is_playing() {
            let block_start_beats = self.transport.position_beats();
            let block_end_beats = block_start_beats
                + Beats(frames as f64 / self.sample_rate as f64 * self.transport.tempo() / 60.0);

            let advance = self.clip_launcher.advance(
                block_start_beats,
                block_end_beats,
                self.transport.tempo(),
                self.sample_rate,
                frames,
            );
            for dispatch in advance.midi {
                self.dispatch_midi(dispatch);
            }
            self.pending_state_changes.extend(advance.state_changes);
        }

        let ctx = ProcessContext {
            buffer_size: frames as usize,
            sample_rate: self.sample_rate,
            tempo: self.transport.tempo(),
            position: self.transport.position(),
            is_playing: self.transport.is_playing(),
        };

        let result = self.processor.process(&mut self.graph, &ctx, &[]);
        output.copy_from(result);

        if self.transport.is_playing() {
            self.transport = self.transport.advance(frames as u64);
        }
    }

    /// Drain any pending slot-state changes (LaunchClip → Queued, advance →
    /// Playing, Stop* → Stopped). The runtime host calls this each block and
    /// forwards each change as `RuntimeEvent::SlotStateChanged`.
    pub fn drain_slot_state_changes(&mut self) -> Vec<SlotStateChange> {
        std::mem::take(&mut self.pending_state_changes)
    }

    /// Read-only access to the clip launcher (slot states for assertions /
    /// inspection from the same thread).
    pub fn clip_launcher(&self) -> &ClipLauncher {
        &self.clip_launcher
    }

    /// Route one clip-launcher MIDI dispatch into the target node's MIDI inbox.
    /// No-op if the target node is missing (e.g., track was deleted while
    /// playback continued — defensive only, the project lifecycle prevents it).
    fn dispatch_midi(&mut self, dispatch: EngineMidiDispatch) {
        if let Some(node) = self.graph.get_node_mut(dispatch.target) {
            node.handle_midi(&dispatch.event, dispatch.sample_offset);
        }
    }

    /// Apply a command to engine state. Called by the audio callback after
    /// draining the command queue.
    pub fn apply_command(&mut self, command: Command) {
        match command {
            Command::Play => {
                self.transport = self.transport.play();
            }
            Command::Stop => {
                self.transport = self.transport.stop();
            }
            Command::SetTempo(bpm) => {
                self.transport = self.transport.set_tempo(bpm);
            }
            Command::SendMidi { target, event, sample_offset } => {
                if let Some(node) = self.graph.get_node_mut(target) {
                    node.handle_midi(&event, sample_offset);
                }
            }
            Command::AddInstrumentChannel { synth_node_id, strip_node_id, track_id } => {
                self.add_instrument_channel(synth_node_id, strip_node_id, track_id);
            }
            Command::RemoveInstrumentChannel { synth_node_id, strip_node_id } => {
                let _ = self.graph.remove_node(synth_node_id);
                let _ = self.graph.remove_node(strip_node_id);
            }
            Command::SetTrackVolume { node_id, volume_db } => {
                self.with_strip(node_id, |s| s.set_volume_db(volume_db));
            }
            Command::SetTrackPan { node_id, pan } => {
                self.with_strip(node_id, |s| s.set_pan(pan));
            }
            Command::SetTrackMute { node_id, muted } => {
                self.with_strip(node_id, |s| s.set_muted(muted));
            }
            Command::SetTrackSolo { node_id, soloed } => {
                self.with_strip(node_id, |s| s.set_soloed(soloed));
                self.recompute_solo_silencing();
            }
            Command::SetMasterVolume { volume_db } => {
                if let Some(node) = self.graph.get_node_mut(self.master_strip_id) {
                    if let Some(master) = node_as_any_mut::<MasterStripNode>(node) {
                        master.set_volume_db(volume_db);
                    }
                }
            }
            Command::LaunchClip { track, scene, playback } => {
                let current_pos = self.transport.position_beats();
                let ts_num = self.transport.time_signature().numerator;
                let outcome =
                    self.clip_launcher
                        .launch_clip(track, scene, playback, current_pos, ts_num);
                for dispatch in outcome.note_offs {
                    self.dispatch_midi(dispatch);
                }
                self.pending_state_changes.extend(outcome.state_changes);
            }
            Command::StopTrack { track } => {
                let outcome = self.clip_launcher.stop_track(track);
                for dispatch in outcome.note_offs {
                    self.dispatch_midi(dispatch);
                }
                self.pending_state_changes.extend(outcome.state_changes);
            }
            Command::LaunchScene { scene, playbacks } => {
                let current_pos = self.transport.position_beats();
                let ts_num = self.transport.time_signature().numerator;
                for (track, playback) in playbacks {
                    let outcome = self.clip_launcher.launch_clip(
                        track,
                        scene,
                        playback,
                        current_pos,
                        ts_num,
                    );
                    for dispatch in outcome.note_offs {
                        self.dispatch_midi(dispatch);
                    }
                    self.pending_state_changes.extend(outcome.state_changes);
                }
            }
            Command::StopAll => {
                let outcome = self.clip_launcher.stop_all();
                for dispatch in outcome.note_offs {
                    self.dispatch_midi(dispatch);
                }
                self.pending_state_changes.extend(outcome.state_changes);
            }
            Command::SetLaunchQuantize(quantize) => {
                self.clip_launcher.quantize = quantize;
            }
        }
    }

    /// Construct a `SynthNode` and a `ChannelStripNode`, wire
    /// `synth.out → strip.in` and `strip.{L,R} → master.{L,R}`.
    /// Idempotent: existing ids are kept, missing ones inserted.
    fn add_instrument_channel(
        &mut self,
        synth_node_id: NodeId,
        strip_node_id: NodeId,
        track_id: crate::ids::TrackId,
    ) {
        // Synth.
        let synth_out_port = if let Some(node) = self.graph.get_node(synth_node_id) {
            node.outputs()[0].id
        } else {
            let synth = SynthNode::with_id(synth_node_id, self.sample_rate);
            let port = synth.outputs()[0].id;
            self.graph.add_node_with_id(synth_node_id, Box::new(synth));
            port
        };

        // Strip.
        let (strip_in_port, strip_l_out, strip_r_out) =
            if let Some(node) = self.graph.get_node(strip_node_id) {
                (
                    node.inputs()[0].id,
                    node.outputs()[0].id,
                    node.outputs()[1].id,
                )
            } else {
                let strip = ChannelStripNode::with_id(strip_node_id, track_id, self.sample_rate);
                let in_id = strip.inputs()[0].id;
                let out_l = strip.outputs()[0].id;
                let out_r = strip.outputs()[1].id;
                self.graph.add_node_with_id(strip_node_id, Box::new(strip));
                (in_id, out_l, out_r)
            };

        // Master strip's input ports.
        let (master_in_l, master_in_r) = {
            let master = self
                .graph
                .get_node(self.master_strip_id)
                .expect("master strip exists");
            (master.inputs()[0].id, master.inputs()[1].id)
        };

        // Wire it up. Errors are idempotent: a duplicate connection is a
        // no-op from the engine's perspective; logged via tracing if available.
        let _ = self.graph.connect(
            PortAddress::new(synth_node_id, synth_out_port),
            PortAddress::new(strip_node_id, strip_in_port),
        );
        let _ = self.graph.connect(
            PortAddress::new(strip_node_id, strip_l_out),
            PortAddress::new(self.master_strip_id, master_in_l),
        );
        let _ = self.graph.connect(
            PortAddress::new(strip_node_id, strip_r_out),
            PortAddress::new(self.master_strip_id, master_in_r),
        );
    }

    /// Helper: run a closure on a `ChannelStripNode` looked up by id, no-op
    /// if the node is absent or not a strip.
    fn with_strip<F: FnOnce(&mut ChannelStripNode)>(&mut self, node_id: NodeId, f: F) {
        if let Some(node) = self.graph.get_node_mut(node_id) {
            if let Some(strip) = node_as_any_mut::<ChannelStripNode>(node) {
                f(strip);
            }
        }
    }

    /// Recompute every channel strip's `silenced_by_other_solo` flag based on
    /// the current set of soloed strips. Called on every `SetTrackSolo`.
    fn recompute_solo_silencing(&mut self) {
        let node_ids: Vec<NodeId> = self.graph.node_ids().collect();

        // Pass 1: collect ids of soloed strips.
        let mut soloed: Vec<NodeId> = Vec::new();
        for id in &node_ids {
            if let Some(node) = self.graph.get_node_mut(*id) {
                if let Some(strip) = node_as_any_mut::<ChannelStripNode>(node) {
                    if strip.is_soloed() {
                        soloed.push(*id);
                    }
                }
            }
        }
        let any_soloed = !soloed.is_empty();

        // Pass 2: set the silencing flag on each strip.
        for id in &node_ids {
            if let Some(node) = self.graph.get_node_mut(*id) {
                if let Some(strip) = node_as_any_mut::<ChannelStripNode>(node) {
                    let is_soloed = soloed.contains(id);
                    strip.set_silenced_by_other_solo(any_soloed && !is_soloed);
                }
            }
        }
    }

    /// Buffer size update for the processor. Called when CPAL changes block size.
    pub fn set_buffer_size(&mut self, buffer_size: usize) {
        self.processor.set_buffer_size(buffer_size);
    }

    /// Transport accessor.
    pub fn transport(&self) -> &Transport {
        &self.transport
    }

    /// Sample rate accessor.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Mutable graph accessor.
    pub fn graph_mut(&mut self) -> &mut AudioGraph {
        &mut self.graph
    }

    /// Graph accessor.
    pub fn graph(&self) -> &AudioGraph {
        &self.graph
    }

    /// Master strip node id.
    pub fn master_strip_id(&self) -> NodeId {
        self.master_strip_id
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new(44100, 512)
    }
}

/// Downcast a `&mut dyn AudioNode` to a concrete type by going through the
/// `as_any_mut` opt-in on the trait.
fn node_as_any_mut<T: AudioNode + 'static>(node: &mut dyn AudioNode) -> Option<&mut T> {
    node.as_any_mut().and_then(|a| a.downcast_mut::<T>())
}
