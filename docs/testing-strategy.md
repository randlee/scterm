# scterm Testing Strategy

## Purpose

This document defines the test layers needed to keep implementation aligned
with the approved requirements and architecture.

## Test Layers

### Unit Tests

Target:

- `scterm-core`

Cover:

- session-name validation
- path resolution
- log-cap parsing
- ancestry derivation and rendering
- packet parsing/serialization
- ring buffer behavior

### Crate Integration Tests

Target:

- `scterm-unix`
- `scterm-app`

Cover:

- socket lifecycle
- PTY lifecycle
- raw-mode restoration
- master/client orchestration
- multi-client behavior
- stale-session handling

### Compatibility Tests

Target:

- whole application behavior

Source of truth:

- `atch/tests/test.sh`
- `compatibility-matrix.md`

Cover:

- commands and aliases
- legacy modes
- default open semantics
- no-TTY behavior
- `current`, `clear`, `list`, `push`, `kill`

### Boundary Tests

Cover:

- no forbidden dependencies in `scterm-core`
- no ATM dependencies outside `scterm-atm`
- no `sc-observability` or external observability crates
- no `ATM_HOME` references

### ATM Extension Tests

Cover:

- blocking receive path
- normalization and sanitization
- dedupe
- exactly-once PTY injection
- carriage-return nudge behavior
- graceful degradation when ATM is unavailable

## Tooling Gates

Required:

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --workspace`

Planned early additions:

- `cargo-audit`
- `cargo-hack`
- `cargo-udeps`
- Miri for any isolated `unsafe` in `scterm-unix`
