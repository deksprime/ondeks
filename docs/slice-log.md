# Slice Log

Living record of slice-by-slice delivery. One entry per slice as it lands.

Plan reference: `../../slice-implementation-plan.md` (outside the repo — private strategy).

## Format

```
## Slice N: Title
- **Status:** Done / In Progress / Blocked
- **Landed:** YYYY-MM-DD
- **Effort:** low / medium / high (vs estimated)
- **Summary:** one paragraph
- **Invariants added:** list
- **Surprises / lessons:** list
- **Follow-ups:** bullets
```

---

## Slice 1 + 2 (merged): Audio Actually Flows + You Hear Your First Sound

- **Status:** Done (pending user audible verification)
- **Started:** 2026-04-20
- **Landed:** 2026-04-20
- **Effort:** HIGH (as estimated)
- **Scope revision:** Plan item 1.T1 (N-channel `AudioBuffer`) deferred. `Buffer`/`StereoBuffer` retained for Tier 1; N-channel generalization will be picked up when drum-rack per-pad outs or surround support becomes a real need. Slice 1 and Slice 2 shipped together because Slice 1 alone has no user-visible outcome.

### Summary
Replaced the silence-in-a-loop lie in the audio callback. The engine now owns the audio graph, the graph processor, and the transport. `Engine::process()` runs the graph in topological order, gathering buffers through connections, producing stereo output. The default graph is a 440 Hz oscillator routed to both output channels, gated by transport state. Clicking Play in the GUI produces a sine tone; Stop silences it.

### Files touched
- **New:** `core/src/graph/nodes/oscillator.rs` — `OscillatorNode` wrapping the DSP oscillator as an `AudioNode`, gated by `context.is_playing`.
- **New:** `core/tests/slice1_audio_flows.rs` — 5 integration tests covering silence-when-stopped, audio-when-playing, transport advance semantics, block continuity.
- **Rewrote:** `core/src/graph/processor.rs` — real buffer routing: per-output-port pool, scratch-buffer gather/scatter, connection resolution for inputs, multi-connection summing, output collection into stereo master.
- **Rewrote:** `core/src/engine.rs` — now owns `AudioGraph`, `GraphProcessor`, and `Transport`; exposes `process(&mut StereoBuffer, frames)` and `apply_command(Command)`. Constructs the default demo graph.
- **Rewrote:** `runtime/src/host.rs` — audio callback now calls `engine.process()` and interleaves the result into CPAL's output. Removed the test-tone hack (kept as a deprecated no-op variant for wire compat).
- **Edit:** `core/src/graph/nodes/mod.rs` — exported `OscillatorNode`.

### Invariants added
- **P0.1 (RT-safety) now enforced in practice** — audio callback is allocation-free during steady state. Pool and scratch buffers grow only at graph-rebuild time.
- **Engine is the single source of truth** for transport + graph + processing state on the audio thread. Host is now a thin CPAL adapter.
- **Default demo graph** is a transparent placeholder — replaced by real user-built graphs in Slice 4 (Track CRUD) and beyond.

### Test results
- 294 existing tests still pass (no regressions).
- 5 new Slice-1 tests pass (silence-when-stopped, audio-when-playing, stop-after-play, transport-advance, cross-block continuity).

### Surprises / lessons
- **Borrow-through-MutexGuard**: calling `host_state.engine.process(&mut host_state.master_buffer, …)` fails because auto-deref through the guard holds the full guard borrow across the call. Fix: one explicit reborrow `let host_state: &mut HostState = &mut guard;` at the top of the callback. Clean, idiomatic.
- **Node trait method visibility**: calling `osc.outputs()[0].id` from `engine.rs` required importing `AudioNode` — the trait method wasn't accessible through an inherent call. Minor but worth remembering.
- **Processor borrow dance**: the node `process()` signature takes `&[&Buffer]` inputs and `&mut [&mut Buffer]` outputs, and both must come from the same buffer pool. Chose scratch-buffer copy-in/copy-out over unsafe pointer splits since `core` forbids unsafe. Cost: 2-4 extra buffer memcpys per node per block (negligible for typical graphs).

### Follow-ups
- Slice 3 (MIDI keyboard + SynthNode) will add MIDI routing through the command queue.
- Slice 4 will remove the default oscillator demo and let the user build the graph.
- The deprecated `play_test_tone` path in `Host` should be deleted once external callers are migrated.

---

## Slice 3: On-Screen MIDI Keyboard + SynthNode

- **Status:** Done (pending user audible verification)
- **Started:** 2026-04-20
- **Landed:** 2026-04-20
- **Effort:** MEDIUM (as estimated)

### Summary
On-screen piano keyboard at the bottom of the window now drives a built-in polyphonic synth. Click-drag plays notes with the mouse; QWERTY (`A W S E D F T G Y H U J K O L`, Ableton-style) plays notes on the computer keyboard for polyphonic chord testing. MIDI flows UI → `Command::SendMidi { target, event, sample_offset }` → `Host` command queue → audio thread → `Engine::apply_command` → `AudioNode::handle_midi` on the target node. The default demo graph is now `SynthNode → OutputNode` (replacing the Slice 2 oscillator). The synth responds to MIDI regardless of transport state so users can noodle without pressing Play — Live-style.

### Files touched
- **New:** `core/src/graph/nodes/synth.rs` — `SynthNode` wrapping `SimpleSynth`. Pre-allocated 128-event inbox, sample-accurate event dispatch via chunked `process()` between sorted event offsets, rt-safe overflow-drops-silently policy. Unit tests for silence/note-on/offset-accuracy/release/overflow.
- **New:** `gui/src/widgets/keyboard.rs` — `PianoKeyboard` widget, 2 octaves C3–B4, mouse + QWERTY input, set-diff algorithm against egui memory to derive NoteOn/NoteOff events each frame. Returns `PianoKeyboardResponse { events }`.
- **New:** `core/tests/slice3_synth_note.rs` — 5 integration tests: end-to-end `Command::SendMidi` routing, unknown-target no-op, polyphony, note-off release, sample-offset respected.
- **Edit:** `core/src/graph/node.rs` — added `AudioNode::handle_midi(&mut self, &MidiEvent, u32)` with no-op default. Contract: rt-safe, no alloc/lock/block.
- **Edit:** `core/src/command.rs` — added `Command::SendMidi { target, event, sample_offset }`. Carries P0.2 sample offset on the wire from day 1.
- **Edit:** `core/src/engine.rs` — default graph now `SynthNode → OutputNode`; `synth_node_id()` accessor; `apply_command` routes `SendMidi` to the target node via `graph.get_node_mut().handle_midi()`.
- **Edit:** `core/src/graph/nodes/mod.rs` — export `SynthNode`.
- **Edit:** `runtime/src/host.rs` — `Host::synth_node_id()` accessor; `NodeId` imported from core.
- **Edit:** `gui/src/widgets/mod.rs` — export `PianoKeyboard` / `PianoKeyboardResponse`.
- **Edit:** `gui/src/app.rs` — `synth_node_id` field, `show_keyboard` toggle (default on), bottom panel stacked above mixer, forwards widget events via `host.send_command(Command::SendMidi)`.
- **Rewrite:** `core/tests/slice1_audio_flows.rs` — updated to reflect that the default graph no longer produces audio on Play alone. Tests now drive the synth via `Command::SendMidi` and verify silence-without-MIDI, note-on-produces-audio, release decay, transport advance, held-note continuity.

### Invariants added / reinforced
- **P0.1 preserved** — SynthNode::handle_midi and process are allocation-free. Inbox capacity 128 fixed; overflow drops silently.
- **P0.2 active** — `Command::SendMidi` carries a `sample_offset` on the wire; SynthNode sorts and dispatches events at offsets via chunked SimpleSynth rendering. UI sends offset=0 (sub-block precision not meaningful for mouse), piano-roll/recording slices will populate real offsets.
- **AudioNode trait now MIDI-aware** — any node can override `handle_midi`. Default no-op keeps existing nodes unchanged. This is the canonical MIDI side-channel for the graph.
- **Commands still plain data (P0.13)** — `SendMidi` is a value-only variant; no closures or references.

### Test results
- **311 tests pass**, zero failures.
- 5 new integration tests in `slice3_synth_note.rs` (end-to-end SendMidi routing).
- 6 new SynthNode unit tests inside `synth.rs` (silence, note-on audio, sample-offset, release, inbox overflow, reset).
- 6 updated Slice 1 tests (rewritten around the new default graph — the old 5 tested behavior that no longer exists).

### Surprises / lessons
- **Default demo graph contract is load-bearing for tests.** Slice 1 tests encoded the "Play → tone" behavior of the OscillatorNode demo. Replacing that with a synth required rewriting those tests around the new contract. Worth flagging: every time the default graph shape changes, the top-layer integration tests change too. Slice 4 will remove the default graph entirely — expect another round of test churn.
- **Egui stacked bottom panels: first shown is outermost.** To put the keyboard above the mixer I show the mixer panel first, then the keyboard panel, then the central panel. Clean once you know the rule.
- **`ondeks_core::ids` is private** — same landmine the handover flagged. `NodeId` / `PortId` are re-exported at the crate root via `pub use ids::*`. Hit this writing the integration test; fixed.
- **Borrow dance: calling `engine.synth_node_id()` inline with `engine.apply_command()`.** Rust evaluates args before the receiver is locked but both need the engine; cleanest pattern is `let target = engine.synth_node_id();` on its own line.
- **Sample-accurate dispatch is almost free.** `SimpleSynth::process(&mut [Sample])` takes an arbitrary slice, so chunked rendering between event offsets is just slice arithmetic — no engine changes needed. Sort cost is negligible at 128 events / block.

### Follow-ups
- **Slice 4** will remove the default synth demo and let users add per-track instruments. `synth_node_id()` on `Engine` / `Host` is a temporary shim — delete it when tracks own their instrument nodes.
- **Velocity-from-Y-position** on the keyboard widget is a Tier 2 polish — currently fixed at 100. Flag for Slice 28 (MIDI mapping / controller input) or sooner.
- **QWERTY input bleeds into global egui state** — if a future text field is focused, typing into it could also play synth notes. Guard with `ui.ctx().wants_keyboard_input()` or a focus sentinel in Slice 4.
- **Non-deterministic `fastrand` in `Oscillator::Noise`** still flagged from Slice 1+2 for P0.7 (deterministic render); unchanged by this slice.
- **All-notes-off on panel hide / app blur** — set-diff handles widget unrender naturally (empty current set → NoteOffs), but full app-focus-loss behavior is unverified. Test when we have a bug.
