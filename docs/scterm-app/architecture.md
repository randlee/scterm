# scterm-app Architecture

## Purpose

This document defines the crate-local architecture of `scterm-app`.

Product architecture is owned by `../architecture.md`. This document covers
the module structure, internal design decisions, and crate-level ADRs for
`scterm-app`.

## Module Responsibilities

The following is the expected module structure. Exact layout is authoritative
in `crates/scterm-app/src/` and `src/` (binary).

- `master` — master loop, PTY ownership, socket server, client dispatch
- `client` — attach client loop, log replay, ring replay, live streaming
- `cli` — command parser, aliases, legacy mode handling
- `commands` — per-command dispatch and wiring
- `messages` — user-facing message rendering
- `logging` — structured logging initialization, `sc-observability` wiring
- `exit` — exit code mapping from `ScError` and app failures

## Dependency Direction

`scterm-app` depends on `scterm-core` and `scterm-unix`.
`scterm-atm` is an optional dependency for Sprint 2 ATM integration.

See `../crate-boundaries.md` for the enforced dependency direction.

## ADR-TERM-APP-001 — PTY Input Serialization Is Mandatory

All PTY input sources (attach client keyboard, `push` command, ATM injection)
share a single serialization point. No two sources may write to the PTY
concurrently.

This is a correctness requirement, not a performance optimization.

## ADR-TERM-APP-002 — Observability Is App-Owned

The `sc-observability` crate is wired at the binary entry point by `scterm-app`.
No lower crate initializes or shuts down the logging subsystem.

## ADR-TERM-APP-003 — anyhow at the Application Boundary Only

`anyhow` is used inside `scterm-app` and the binary. It is not re-exported and
is not used in `scterm-core` or `scterm-unix`.
