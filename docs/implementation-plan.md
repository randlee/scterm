# scterm Implementation Plan

## Purpose

This document defines the implementation order that preserves architecture
boundaries and minimizes rework.

## Implementation Rule

Do not start a later phase if its prerequisites are still ambiguous in the
docs.

## Phase 0: Workspace Guardrails

Deliver:

- workspace crate skeleton
- lint policy
- formatting policy
- initial CI commands
- boundary docs checked into the repo

Done when:

- crate layout matches `crate-boundaries.md`
- dependency policy is reflected in `Cargo.toml`
- all three Sprint 1 crates (`scterm-core`, `scterm-unix`, `scterm-app`)
  compile as empty lib crates
- no code exists outside the approved crate plan

## Phase 1: Core Contracts

Deliver in `scterm-core`:

- `SessionName`, `SessionPath`, `LogCap`, `RingSize`
- session ancestry helpers
- client-to-master packet model
- ring buffer
- core state-machine types
- domain error surface

Done when:

- core compiles without Tokio, PTY, or Unix socket dependencies
- unit tests cover path validation, size parsing, ancestry handling, packet
  parsing, and ring-buffer behavior

## Phase 2: Unix Runtime

Deliver in `scterm-unix`:

- Unix socket bind/connect/listen/accept
- PTY spawn/read/write/resize
- raw-mode terminal guard
- signal handling
- daemonization support

Done when:

- runtime traits are sealed
- Unix-only integration tests cover socket lifecycle, PTY lifecycle, and raw-mode restoration

## Phase 3: Session Orchestration

Deliver in `scterm-app`:

- master loop
- attach client loop
- session log replay
- ring replay orchestration
- PTY input serialization
- structured logging setup via `AppLogger` (serde_json + std::io)
- no active Sprint 1 output-observer behavior beyond a reserved passive hook

Done when:

- master is the only PTY owner
- app owns logger lifecycle and structured logging configuration
- lower crates remain logging-implementation-agnostic
- any reserved output-observer hook is passive, app-layer only, and carries no
  Sprint 1 behavioral obligations

## Phase 4: CLI and Compatibility Surface

Deliver:

- command parser
- aliases
- legacy single-letter modes
- default open behavior
- user-facing messages and exit codes

Done when:

- command semantics match `compatibility-matrix.md`
- no CLI rule is duplicated in lower crates

## Phase 5: Compatibility Test Closure

Deliver:

- Rust compatibility tests derived from `atch/tests/test.sh`
- Unix integration coverage for stale sockets, log replay, kill, clear, and
  ancestry behavior

Done when:

- Sprint 1 acceptance criteria from `requirements.md` are met

## Phase 6: ATM Bridge

Deliver in `scterm-atm` and `scterm-app`:

- blocking `atm` read integration
- typed inbound message events
- dedupe
- sanitized PTY injection with trailing carriage return

Done when:

- ATM remains optional
- no ATM crate or `ATM_HOME` dependency exists
- exactly-once injection behavior is tested
