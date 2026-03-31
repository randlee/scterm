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
- `sc-observability` in `scterm-unix` directly (observability is app-layer only)

### `scterm-app`

Allowed:

- `sc-observability` and `sc-observability-types` (structured logging backend)
- `time` (for `OffsetDateTime::now_utc()` in `LogEvent` construction)
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
- `sc-observability` (observability initialization belongs to `scterm-app`)

## Observability Dependency Policy

Structured logging policy:

- `scterm-app` depends on `sc-observability` as the JSONL logging backend.
- `scterm-app` owns the logger lifecycle; lower crates do not initialize or
  shut down the logging subsystem.
- `scterm-core` and `scterm-unix` are logging-implementation-agnostic. They
  do not depend on `sc-observability` or any logging backend.
- The `SC_LOG_ROOT` environment variable is the cross-tool log root convention.
  The ATM app layer sets this at launch so `scterm` and `schook` logs land in a
  consistent location. `scterm` never reads `ATM_HOME` directly.

OTel path:

- `sc-observability-otlp` is the planned OTel export layer. When that
  requirement matures, it is wired in `scterm-app` only; lower layers remain
  unaffected.

Dependency version constraint:

- `sc-observability` 0.45.x (published) depends on `agent-team-mail-core`
  (ATM boundary violation). Use `sc-observability` 0.46.x via path dep until
  0.46.x is published to crates.io.

## CI Enforcement Targets

Future CI should validate:

- no forbidden internal dependency edges
- no `agent-team-mail-*` dependencies
- no ATM Rust imports
- no `ATM_HOME` references
- no `sc-observability` in `scterm-core`, `scterm-unix`, or `scterm-atm`
- no runtime crates in `scterm-core`
