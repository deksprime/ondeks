# Ondeks DAW Engine

A professional-grade Digital Audio Workstation (DAW) engine written in Rust.

## Overview

Ondeks is designed as a modular, testable, high-performance audio engine that can power multiple interfaces including CLI, native GUI, and eventually browser-based applications.

## Goals

- **Performance**: Real-time audio processing with < 5ms latency capability
- **Testability**: Every component testable in isolation without audio hardware
- **Modularity**: Clear separation between pure logic, runtime, and interfaces
- **Extensibility**: Plugin architecture for custom instruments and effects
- **Portability**: Core engine runs anywhere Rust compiles; runtime adapts to platform

## Architecture

The project is organized into three main layers:

1. **Core** (`crates/core`): Pure audio logic, deterministic processing
2. **Runtime** (`crates/runtime`): Real-world integration, threading, IO
3. **CLI** (`crates/cli`): Command-line interface

See [docs/architecture.md](docs/architecture.md) for detailed architecture documentation.

## Building

```bash
cargo build
```

## Running

```bash
cargo run -p ondeks-cli
```

## Testing

```bash
cargo test
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

## License

MIT
