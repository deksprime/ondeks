# Ondeks Architecture

## Layer Model

```
┌─────────────────────────────────────────────────────────────────┐
│                        LAYER 3: INTERFACES                       │
│  Responsibility: User interaction, visualization, external APIs  │
│  Examples: CLI, GUI (egui/Tauri), HTTP API, Scripting           │
│  Crate: cli (initially), gui (future)                           │
└─────────────────────────────────────────────────────────────────┘
                                 │
                                 ▼ Commands / Queries
┌─────────────────────────────────────────────────────────────────┐
│                        LAYER 2: RUNTIME                          │
│  Responsibility: Real-world integration, threading, IO          │
│  Examples: Audio callbacks, MIDI IO, file loading, scheduling   │
│  Crate: runtime                                                 │
│  Dependencies: cpal, midir, crossbeam, hound                    │
└─────────────────────────────────────────────────────────────────┘
                                 │
                                 ▼ Pure function calls
┌─────────────────────────────────────────────────────────────────┐
│                        LAYER 1: CORE                             │
│  Responsibility: Pure audio logic, deterministic processing     │
│  Examples: Graph evaluation, MIDI processing, DSP, mixing       │
│  Crate: core                                                    │
│  Dependencies: NONE (except serde for serialization)            │
└─────────────────────────────────────────────────────────────────┘
```

## Crate Dependency Graph

```
cli ──────► runtime ──────► core
               │
               ▼
         [external crates]
         - cpal (audio IO)
         - midir (MIDI IO)
         - crossbeam (lock-free queues)
         - hound (WAV file IO)
         - serde (serialization)
```

## Data Flow

```
User Input (CLI/GUI)
        │
        ▼
   ┌─────────┐
   │ Command │  (e.g., Play, AddTrack, SetTempo)
   └────┬────┘
        │
        ▼
┌───────────────┐
│ Command Queue │  Lock-free, non-RT to RT
└───────┬───────┘
        │
        ▼
┌───────────────┐
│  Audio Thread │  Real-time, processes commands
│   (Runtime)   │
└───────┬───────┘
        │
        ▼
┌───────────────┐
│  Core Engine  │  Pure: (State, Command) → (State, Events)
└───────┬───────┘
        │
        ▼
┌───────────────┐
│  Event Queue  │  Lock-free, RT to non-RT
└───────┬───────┘
        │
        ▼
   ┌─────────┐
   │  Event  │  (e.g., PlaybackStarted, ClipTriggered, MeterUpdate)
   └────┬────┘
        │
        ▼
   UI Update / Logging
```

## Design Principles

### Separation of Concerns

- **Core**: Pure, deterministic audio processing logic. No IO, no threading, no side effects.
- **Runtime**: Handles all real-world integration (audio hardware, MIDI devices, file systems).
- **CLI**: User interface layer that orchestrates runtime and presents results.

### Testability

Every component in the core crate can be tested without audio hardware or external dependencies. The runtime layer provides test doubles for integration testing.

### Real-Time Safety

The core engine is designed to be called from real-time audio threads. It uses no allocations, no locks, and no blocking operations.
