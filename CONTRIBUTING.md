# Contributing to Ondeks

Thank you for your interest in contributing to Ondeks!

## Development Principles

### Test-Driven Development

Every task follows this pattern:
1. Define the interface — What types and functions exist?
2. Write the tests — What should the behavior be?
3. Implement — Make the tests pass
4. Refactor — Clean up while tests stay green

### Commit Granularity

Each task should result in one or more atomic commits:
- Commit compiles (no broken builds)
- Commit has passing tests
- Commit message references task ID (e.g., `[P1-T3] Implement Buffer::silence()`)

### Documentation Standard

Every public item must have:
- A doc comment explaining **what** it does
- For complex items, **why** it exists and **how** to use it
- Examples where non-obvious

## Code Style

- Follow Rust standard formatting: `cargo fmt`
- Run clippy: `cargo clippy --all-targets --all-features -- -D warnings`
- All code must compile with `#![forbid(unsafe_code)]` in the core crate

## Testing

Run all tests:
```bash
cargo test --all-targets
```

Run tests for a specific crate:
```bash
cargo test -p ondeks-core
```

## Project Structure

- `crates/core/` - Pure audio engine logic (no external dependencies except serde)
- `crates/runtime/` - Audio/MIDI IO, threading, real-time integration
- `crates/cli/` - Command-line interface

See the [implementation plan](implementation-plan.md) for detailed task breakdowns.
