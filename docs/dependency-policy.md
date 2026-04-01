# scterm Dependency Policy

## Purpose

This document defines the allowed dependency graph for the `scterm` workspace.

The goal is to make drift obvious and reviewable.

## Internal Dependency Graph

Required direction:

```text
scterm-core
    ^
    |
scterm-unix
    ^
    |
scterm-app

Sprint 2:
scterm-core <- scterm-atm
                 ^
                 |
            scterm-app
```

Allowed edges:

- `scterm-unix -> scterm-core`
- `scterm-app -> scterm-core`
- `scterm-app -> scterm-unix`
- `scterm-atm -> scterm-core`
- `scterm-app -> scterm-atm`

Forbidden edges:

- `scterm-core -> scterm-unix`
- `scterm-core -> scterm-app`
- `scterm-core -> scterm-atm`
- `scterm-unix -> scterm-app`
- `scterm-unix -> scterm-atm`
- `scterm-atm -> scterm-unix`

## External Dependency Rules

### `scterm-core`

Allowed:

- standard-library-centric helpers
- error and derive helpers with minimal surface area

Forbidden:

- async runtimes
- PTY libraries
- Unix socket libraries
- CLI parser crates
- logging backends
- ATM tooling crates

### `scterm-unix`

Allowed:

- Unix socket and PTY dependencies
- signal and terminal-control dependencies

Forbidden:

- ATM libraries
- `sc-observability` or any crate from the sibling `sc-observability` workspace

### `scterm-app`

Allowed:

- `serde_json` (via AppLogger — no external observability crate)
- CLI parsing crate
- one application error crate

### `scterm-atm`

Allowed:

- process-execution support for the external `atm` CLI
- parsing helpers
- local persistence helpers for dedupe state

Forbidden:

- ATM Rust crates
- PTY and Unix runtime internals from `scterm-unix`
- any higher-layer crate from the sibling `sc-observability` workspace

## Observability Dependency Policy

Structured logging policy:

- use the self-contained `AppLogger` in `scterm-app` (serde_json + std::io) only
- no external observability crate dependency is required or permitted in this repo
- keep observability as local structured logging rather than a broader event bus

Rationale:

- structured logs are immediately useful for implementation and debugging
- a standalone AppLogger avoids all external observability crate dependencies
- additional observability layers are intentionally deferred from this repo’s
  approved design docs

## CI Enforcement Targets

Future CI should validate:

- no forbidden internal dependency edges
- no `agent-team-mail-*` dependencies
- no ATM Rust imports
- no `ATM_HOME` references
- no `sc-observability` or external observability crate in any manifest
- no runtime crates in `scterm-core`
