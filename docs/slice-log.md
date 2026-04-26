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

---

## Slice 4: Track CRUD From UI

- **Status:** Done (pending user verification)
- **Started:** 2026-04-20
- **Landed:** 2026-04-20
- **Effort:** MEDIUM-HIGH (as estimated)

### Summary
The app now opens to a real, empty project (master + Scene 1 only — the `Bass/Lead/Drums/Vocals` demo block is gone). The "+ Track" button in the session header adds MIDI or Audio tracks. Right-clicking a track header opens a context menu with Rename (inline TextEdit), Duplicate, Delete, Move Up/Down, and a Color submenu (8-color palette). Every mutation records its inverse in `History`; Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z undo and redo cleanly, round-tripping back to the exact prior state — including preserving track IDs across undo→redo cycles so listeners (Slice 5's instrument routing, future save/load) stay stable.

### Architectural decision: deferred "Engine owns Project"
The plan (4.T2) prescribed moving `Project` into the engine so the audio thread becomes the single source of truth. That conflicts with **P0.1** — mutating `project.tracks` (e.g. `Vec::push`) allocates, and `Engine::apply_command` runs on the audio thread. Resolution options were (a) keep Project on UI thread with a dispatcher, (b) put Project behind the existing `HostState` mutex, or (c) fully event-sourced snapshots into UI. Picked (a): Project lives on the UI thread; `apply_project_command()` mutates and returns an inverse. Engine-side changes (adding/removing per-track instrument nodes) will be dispatched alongside via a distinct engine command path in Slice 5. This is documented in code with explicit comments on `OndeksApp::project` and on the `SynthNode` / `synth_node_id` shim.

### Files touched
- **New:** `ui-common/src/dispatch.rs` — `apply_project_command()` + `apply_without_outcome()`. Single dispatch point for all project mutations with undo; pure function over `&mut Project`. 7 inline unit tests covering every variant's inverse.
- **New:** `ui-common/tests/slice4_track_crud.rs` — 7 end-to-end history tests (add/remove/rename/duplicate/move round-trips, cascaded undo, master removal rejected).
- **Edit:** `ui-common/src/commands/project.rs` — added `ProjectCommand::RestoreTrack { track, insert_at }` (undo-only, carries a full `Box<Track>` snapshot so delete can be reversed exactly). `description()` labels wired for each variant.
- **Edit:** `ui-common/src/lib.rs` — export the new dispatch module.
- **Edit:** `core/src/project/project.rs` — added `insert_track_at(track, insert_at)`, `move_track(id, new_index)`, `track_index(id)`. `remove_track` now returns `(Track, usize)` so the dispatcher can capture state for undo.
- **Edit:** `core/src/error.rs` — added `ProjectError::Unsupported(String)` for the dispatcher's deferred command branches (Save/Load/Scene CRUD, landing in later slices).
- **Edit:** `gui/src/views/session.rs` — track-header right-click context menu, inline rename with TextEdit, "+ Track" popup with MIDI/Audio submenu, new `TrackMenuAction` enum + `SessionViewResponse` fields (`track_menu_action`, `add_track_clicked`, `rename_commit`, `rename_cancel`).
- **Edit:** `gui/src/app.rs` — `dispatch_project_command` and `apply_ui_command_replay` helpers, `handle_undo_redo_shortcuts` (egui `consume_shortcut` for Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z), inline-rename buffer field, demo-data block deleted, undo/redo status labels in toolbar, session view wired to dispatcher.

### Invariants added / reinforced
- **P0.6 undo discipline operational** — every project-mutating command is paired with an inverse via `ApplyOutcome { undo, redo }`. The dispatcher is the one-way funnel; no code path mutates `Project` fields directly.
- **Deterministic redo** — for commands that generate new IDs (`AddTrack`, `DuplicateTrack`), the `redo` command is a `RestoreTrack` carrying the exact track snapshot created on first execution. Undo→redo preserves track IDs bit-identically, which Slice 5 (per-track instrument nodes) will need when reattaching graph nodes to track IDs.
- **Master track immovable / irremovable** — enforced in `Project::remove_track` (already present) and additionally in `insert_track_at` (rejects `TrackType::Master`) and `move_track` (rejects master id).

### Test results
- **325 tests pass**, zero failures (up from 311).
- 7 new dispatcher unit tests in `ui-common/src/dispatch.rs`.
- 7 new integration tests in `ui-common/tests/slice4_track_crud.rs`.
- All prior Slice 1-3 and phase-1-9 tests unchanged.

### Surprises / lessons
- **Inline rename focus is quirky.** `edit_resp.request_focus()` fires every frame the widget exists, which works but can look flickery if the rename buffer's lifetime spans multiple frames. Using a buffer on `OndeksApp` (outside SessionView's transient state) keeps the TextEdit's egui-side identity stable so focus stays put. The simpler alternative (memory-based transient state inside SessionView) was tempting but broke focus tracking.
- **`context_menu` closure can't directly mutate `SessionViewResponse`.** Borrow checker objects to the response being mutably borrowed inside the closure and read outside it. Pattern: declare a local `Option<TrackMenuAction>` in the loop body, mutate inside the closure, assign to `response.track_menu_action` after the closure returns.
- **Redo determinism is load-bearing for future slices.** Initially I had redo just re-apply the forward command, but AddTrack → undo → redo would generate a different `NodeId`. Switched to `RestoreTrack { snapshot }` for redo-of-add. Slice 5 cares: graph nodes are keyed by `TrackId`, so those IDs need to persist through undo→redo.
- **`ProjectError::Unsupported`** was cheaper than refactoring the Result type in the dispatcher to a custom error enum. When Save/Load/Scene-CRUD slices land, this variant will probably get replaced by real error types — flag for a small cleanup pass then.
- **`#[allow(dead_code)]` on `SessionViewResponse::selection_changed` was tempting but skipped** — the field is there because the ShortcutMap infrastructure wants it; leaving the warning surfaces unfinished wiring so it gets wired when selection actions need it.

### Follow-ups
- **Slice 5** builds on this: when `AddTrack` runs, the engine also needs to create a per-track `SynthNode`. That's a new engine command `Command::AddInstrumentNode { track_id, kind }`. The existing default demo synth + `synth_node_id()` shim must die in the same slice (otherwise there are two synths competing for keyboard input).
- **Scene CRUD** deferred. The dispatcher has stub arms that error with `Unsupported`.
- **Multi-select / group operations on tracks** — the current dispatcher is per-track. Batch commands (e.g. delete 3 tracks with one undo) will want a `UiCommand::Batch(Vec<UiCommand>)` wrapper. Flag for Slice 8 piano roll where multi-select notes need the same treatment.
- **Save/Load** still missing (Slice 7). Right now if the app crashes mid-session all work is lost; worth flagging to user when they start a real session.
- **Color palette is fixed at 8 presets.** A proper color picker (HSV wheel or RGB sliders) is a polish slice; for now 8 is enough to distinguish tracks visually.
- **Undo/redo description labels in toolbar are faint gray** — the labels show whatever command description last ran. Useful for debugging, possibly noisy for regular users; hide behind a preference later.

### Post-landing fix (same day)
- **Ctrl+Shift+Z / Ctrl+Y silently swallowed by the undo shortcut.** egui's `Modifiers::matches_logically` treats the pattern's modifier flags as a *subset* requirement rather than an exact match — so the undo pattern `Ctrl+Z` (no shift) matched `Ctrl+Shift+Z` events, extra Shift was ignored, `consume_shortcut(&undo)` ate the event before redo's pattern could see it. Fix: replace `consume_shortcut` with an explicit `i.events.retain(...)` scan that checks modifier equality exactly. Added tracing::debug!() on both undo and redo replay paths so future diagnoses are easier.

---

## Slice 5: Synth Instrument Per MIDI Track

- **Status:** Done (pending user verification)
- **Started:** 2026-04-20
- **Landed:** 2026-04-20
- **Effort:** MEDIUM (as estimated)

### Summary
Each MIDI track now owns its own `SynthNode`. The keyboard widget targets the **armed** track's instrument — arm a track via the new "R" button in the session header, play the keyboard, only that track's synth sounds. Adding a MIDI track automatically creates its synth node with a pre-assigned `NodeId` that matches the `Track::instrument` field; removing a track removes its node; undo/redo round-trips the same node id so the graph is byte-identical across history cycles. The Slice 3 default-demo-graph shim (`Engine::synth_node_id` / `Host::synth_node_id`) is gone — `Engine::new()` now produces an empty graph (master only).

### Files touched
- **Edit:** `core/src/project/track.rs` — `Track::instrument: Option<NodeId>` field, populated with a fresh id for MIDI tracks at construction. Ensures graph node identity and `Track::instrument` are the same id.
- **Edit:** `core/src/project/project.rs` — `arm_exclusive(id) -> Option<TrackId>` (returns previously-armed id), `disarm_all()`, `armed_track() -> Option<&Track>`.
- **Edit:** `core/src/command.rs` — new `Command::AddSynthNode { node_id, track_id }` and `Command::RemoveSynthNode { node_id }` variants.
- **Edit:** `core/src/graph/nodes/synth.rs` — `SynthNode::with_id(id, sample_rate)` constructor; `SynthNode::new` now delegates to it with a fresh id. Lets the dispatcher pre-assign node ids that match the Track's instrument field.
- **Rewrite:** `core/src/engine.rs` — `Engine::new()` builds an **empty** graph (master only — no default demo synth). Removed `Engine::synth_node_id()`. Added internal `add_synth_node(id)` / `remove_synth_node(id)` helpers that `apply_command` dispatches to.
- **Edit:** `runtime/src/host.rs` — removed `Host::synth_node_id()` and the `NodeId` import.
- **Edit:** `ui-common/src/commands/project.rs` — added `ProjectCommand::ArmTrack { track_id }` (explicitly **non-undoable**; echoes back as both undo and redo so if something does push it into history it stays idempotent).
- **Rewrite:** `ui-common/src/dispatch.rs` — `ApplyOutcome` grows `engine_commands: Vec<EngineCommand>`. `engine_commands_for_track()` emits `AddSynthNode` / `RemoveSynthNode` keyed on `Track::instrument` (skipped for non-MIDI tracks). DuplicateTrack generates a *fresh* instrument id for the duplicate so each duplicate gets its own graph node. ArmTrack branch is non-mutating-to-undo (echoed as self). Added/updated dispatcher tests covering engine-command emission.
- **Edit:** `gui/src/views/session.rs` — added 14×14 "R" arm button sub-rect in the upper-right of each MIDI track's header. Red when armed, gray otherwise. Emits `SessionViewResponse::arm_track`. Non-MIDI tracks don't render the button (they can't receive MIDI).
- **Edit:** `gui/src/app.rs` — `dispatch_project_command` now forwards every `ApplyOutcome::engine_commands` entry to the Host. `apply_ui_command_replay` does the same so undo/redo keeps the graph in sync. Removed the `synth_node_id` field and init. `render_keyboard` routes `Command::SendMidi` to `project.armed_track().instrument`, silent when no track is armed. Arm toggling: clicking the R button on an armed track disarms (non-undoable); clicking an unarmed track dispatches `ArmTrack` (exclusive).
- **Rewrite:** `core/tests/slice1_audio_flows.rs`, `core/tests/slice3_synth_note.rs` — both now construct an engine then add a synth via `Command::AddSynthNode` before sending MIDI (`new_engine_with_synth()` helper). Added `empty_graph_is_silent` test for Slice 1 to pin the new default.
- **New:** `core/tests/slice5_per_track_instrument.rs` — 6 tests: default engine has an empty graph, add-synth produces audio, remove-synth silences, two synths are independent (sum louder), duplicate-id add is a noop, remove of unknown node is a noop.

### Invariants added / reinforced
- **Tracks own their instrument** — `Track::instrument` is the single source of truth for which graph node represents a MIDI track. `NodeId` generation happens at Track construction; the dispatcher reuses that id via `AddSynthNode` so graph and project agree bit-identically.
- **Engine graph is derived state** — the engine holds no default graph beyond master output; everything is added via `Command::AddSynthNode` in response to UI actions. Slice 7 (Save/Load) will walk tracks and re-emit AddSynthNode on project load.
- **Arm is exclusive and ephemeral** — only one track armed at a time; arm state is *not* undoable (matches transport Play/Stop invariant).
- **Undo/redo preserves graph identity** — the same `NodeId` lives through remove→restore cycles because `RestoreTrack` carries the full Track snapshot including its `instrument` field. The dispatcher re-emits `AddSynthNode` with that preserved id.

### Test results
- **337 tests pass**, zero failures (up from 325).
- 12 new tests: 6 slice5_per_track_instrument + 3 new dispatcher tests for engine-command emission + 3 updated Slice 1/3 tests (empty graph silence, add-synth-with-helper pattern).
- Slice 1 and Slice 3 tests now go through the full `AddSynthNode → SendMidi` dispatch path, which exercises more of the engine than the previous "implicit default graph" setup.

### Surprises / lessons
- **`Track::instrument` at construction, not at dispatch.** Initially I had the dispatcher generate the `NodeId` when `AddTrack` ran. But `Track::new` is called from other places (tests, CLI, eventually Load). Making the Track self-sufficient (owns its id) keeps invariants local — any new Track that should have an instrument gets one for free.
- **Redo of duplicate needs a *separate* instrument id snapshot.** For `DuplicateTrack`, the duplicate is a fresh Track with a fresh `NodeId`. The redo (`RestoreTrack { snapshot }`) must snapshot *that* duplicate, not the source — otherwise undo→redo would create different graph nodes. Caught it with a test that's now in `dispatch.rs`.
- **Engine methods need to be no-ops on duplicate/missing ids.** `AddSynthNode` may arrive twice if a user redoes rapidly; `RemoveSynthNode` may arrive for an already-gone node. Both log-and-continue rather than panic keeps the audio thread alive during racy command sequences.
- **Session view's arm button hit-test sits in front of the header rect's interact.** Allocating the header's response with `Sense::click_and_drag` claims the whole rect; the arm sub-rect uses `ui.interact(sub_rect, id, Sense::click())` which egui lets us layer. The layering relies on the arm rect being rendered *after* the header paint, so paint order matters.
- **Silently dropping keyboard MIDI when no track is armed** is the right call (matches Ableton). An earlier draft would have warned via tracing, but it fires every frame the user tries to noodle without arming → noisy log. Dropped silently instead; the UI's missing-audio cue is already "no sound."

### Follow-ups
- **Slice 6 (working mixer)** is next: volume faders / pan / mute / solo actually affect audio. Mixer will want a `ChannelStripNode` inserted between each instrument and master. At that point `AddSynthNode` might evolve to `AddInstrumentChannel` that wires `SynthNode → ChannelStripNode → Master` atomically.
- **Non-MIDI track types still have no audio representation.** Audio, Group, Return tracks exist in the model but don't have graph nodes. Slice 11+ (Load Audio File → Clip) will introduce `AudioClipPlayerNode`; Group/Return wait until sends (Slice 23).
- **Arm state survives undo of the armed track's deletion.** If you arm track A, delete A, undo → A is back but the `armed` flag is false (since the snapshot captured it pre-arm). Acceptable; users re-arm when they want to play. Could be revisited if annoying.
- **The Track::instrument field on audio/return/group tracks is always None.** If we later introduce audio track instruments (e.g. for a Simpler sampler on an Audio track? Unlikely but possible), widen this then.
- **Arm is not yet keyboard-accessible.** You have to click the R button; no shortcut like Alt+Number. Flag for polish.

---

## Slice 6: Working Mixer

- **Status:** Done (pending user verification)
- **Started:** 2026-04-26
- **Landed:** 2026-04-26
- **Effort:** MEDIUM (as estimated)

### Summary
Volume faders, pan, mute, solo, and master fader actually affect audio. Each MIDI track now flows `SynthNode → ChannelStripNode → MasterStripNode → OutputNode`. Per-track strip exposes smoothed gain (~50 ms) and constant-power pan; mute/solo gate the gain target so transitions don't click. Solo is exclusive in semantics (any soloed strip silences all non-soloed strips); the engine recomputes silencing on every solo change. Master strip is a stereo-in/stereo-out gain stage between all per-track strips and the OutputNode.

### Files touched
- **New:** `core/src/graph/nodes/channel_strip.rs` — `ChannelStripNode` (1 mono in → 2 mono outs). One-pole smoothing on gain and pan against per-block targets. Mute/solo gate folded into the gain target so it ramps. 5 unit tests including click avoidance.
- **New:** `core/src/graph/nodes/master_strip.rs` — `MasterStripNode` (2 mono ins → 2 mono outs). Smoothed master gain; uses the graph processor's existing multi-connection summing on its input ports.
- **Edit:** `core/src/graph/node.rs` — added opt-in `AudioNode::as_any_mut() -> Option<&mut dyn Any>`. Default `None`; `ChannelStripNode` and `MasterStripNode` override to return `Some(self)` so the engine can downcast trait objects when handling mixer commands.
- **Edit:** `core/src/graph/nodes/mod.rs` — export the new nodes.
- **Edit:** `core/src/project/track.rs` — `Track::channel_strip: Option<NodeId>` alongside `instrument`. Both ids are pre-allocated for MIDI tracks at construction so undo/redo preserves both node identities.
- **Rewrite:** `core/src/command.rs` — `Command::AddSynthNode` / `RemoveSynthNode` (Slice 5) renamed to `AddInstrumentChannel { synth_node_id, strip_node_id, track_id }` / `RemoveInstrumentChannel { synth_node_id, strip_node_id }`. Added mixer commands: `SetTrackVolume`, `SetTrackPan`, `SetTrackMute`, `SetTrackSolo`, `SetMasterVolume`.
- **Rewrite:** `core/src/engine.rs` — `Engine::new` now creates `MasterStripNode → OutputNode`; per-track `add_instrument_channel` builds `Synth → Strip → Master` atomically. Mixer commands route through a `with_strip` helper using the new `as_any_mut` downcast. Solo recomputes `silenced_by_other_solo` on every strip after any solo change (two-pass: collect soloed, then set the flag).
- **Edit:** `ui-common/src/dispatch.rs` — emits `AddInstrumentChannel` / `RemoveInstrumentChannel` keyed on `Track::{instrument, channel_strip}`. `DuplicateTrack` allocates fresh ids for both. Updated tests to assert on the new variants.
- **Edit:** `ui-common/src/commands/project.rs` — descriptions unchanged; `ProjectCommand` itself untouched (mixer state lives on `Track` directly, not as `ProjectCommand` variants — kept simple for Slice 6, may revisit if mixer changes need to participate in undo).
- **Rewrite:** `gui/src/app.rs::render_mixer` — per-track strips with vertical fader, horizontal pan slider, M/S buttons, dB/pan labels. Master strip rendered on the right. All controls dispatch real engine commands and mutate project state alongside (no undo for mixer changes in Slice 6 — coalescing drag history is polish).
- **Rewrite:** `core/tests/slice5_per_track_instrument.rs` — uses `AddInstrumentChannel` and `RemoveInstrumentChannel`; `default_engine_has_master_strip_and_output` asserts the new 2-node baseline (master strip + output).
- **Rewrite:** `core/tests/slice1_audio_flows.rs`, `core/tests/slice3_synth_note.rs` — new helper `new_engine_with_synth()` constructs the engine and dispatches `AddInstrumentChannel`.
- **New:** `core/tests/slice6_mixer.rs` — 5 integration tests: fader attenuates a track, mute silences a track, solo silences other tracks, pan-full-left silences right channel, master fader attenuates everything. Each asserts on tail-of-buffer (not peak) so the smoothing ramp doesn't dominate.

### Invariants added / reinforced
- **Click-free parameter changes** — gain and pan are smoothed via a one-pole low-pass against per-block targets with a ~50 ms time constant. Step changes (mute, solo gating) fold into the gain target, so they ramp.
- **Solo is exclusive in semantics, not in storage** — multiple strips can have `soloed = true`, but the engine treats that as "any of these are soloed → silence the rest". Two-pass recomputation runs on every `SetTrackSolo` command.
- **Strip and instrument node ids on Track are stable across undo/redo** — `Track::instrument` AND `Track::channel_strip` are pre-allocated at construction. The dispatcher's `RestoreTrack` snapshot carries both, so undo→redo cycles produce a graph that's byte-identical to the pre-undo state.
- **`AudioNode::as_any_mut` is opt-in** — default returns `None`, so existing nodes that don't need to be poked outside the trait interface pay zero cost. Only `ChannelStripNode` and `MasterStripNode` opt in.

### Test results
- **349 tests pass**, zero failures (up from 337).
- 5 new strip unit tests + 5 new Slice 6 integration tests + 2 master strip tests + updates to Slice 1/3/5 tests.

### Surprises / lessons
- **`Buffer::peak()` over a smoothing ramp is misleading.** First draft of the smoothing tests asserted `peak < target` over a buffer that contained the full ramp — peak was the start of the ramp, which is high. Switched all settle tests to look at the tail (last 256 samples) instead. Lesson: when testing smoothed parameter changes, assert on what you're settling *to*, not the peak across the transition.
- **`AudioNode` trait objects can't auto-downcast through `Any`.** Adding a non-default `as_any_mut(&mut self) -> Option<&mut dyn Any>` method to the trait is the canonical safe-Rust opt-in. Default `None` means existing nodes don't change; mixer nodes return `Some(self)`. Tried fancier tricks (raw `Any` cast, deferred dispatch tables) — opt-in default is by far the cleanest.
- **Borrow-splitting `&mut [&mut Buffer]`.** To touch both L and R output buffers in the same iteration, the cleanest pattern is `match &mut *outputs.audio { [l, r, ..] => (&mut **l, &mut **r), _ => return }`. Slice destructure + reborrow. Tried `split_at_mut`, `audio_mut(0)` + `audio_mut(1)` — both fight the borrow checker.
- **Slice 6 broke 11 existing tests** — every Slice 1/3/5 test that referenced `AddSynthNode`/`RemoveSynthNode` had to be updated, plus the engine-default-graph baseline changed from 1 node (output) to 2 (output + master strip). Worth flagging that every change to default graph topology cascades; the dispatcher's `engine_commands_for_track` is the choke point that absorbs most of it.
- **Master fader without an undo entry feels right** — egui's slider fires `.changed()` on every drag tick, and a per-tick history entry is awful UX. Mixer mutations skip history entirely in Slice 6. Drag-coalesced undo for faders is a polish slice.

### Follow-ups
- **Slice 7 (Save/Load)** is next. The full project state (tracks, clips, mixer values) must round-trip through JSON. P0.3 versioning framework operational. Slice 7 will need to walk the project on load and dispatch `AddInstrumentChannel` for each MIDI track to rebuild the graph.
- **Mixer changes don't go through undo.** Acceptable for MVP, but a polish slice should add drag-coalesced history entries (one per gesture between mouse-down and mouse-up).
- **Per-track meters** — strip nodes don't yet emit their own meter readings. The runtime master meter still works via `Host`'s post-output peak, but per-strip metering is a separate slice.
- **No master mute or solo** — master strip only has a gain. Adding a mute on the master is trivial if needed.
- **Pan law is constant-power.** For mono → stereo this is canonical; if Slice 11+ introduces stereo-input audio tracks, we'll need a "balance" pan (cuts opposite channel) vs. "true pan" (Haas-style) decision.
