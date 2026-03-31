# scterm-core Architecture

## Purpose

This document defines the crate-local architecture of `scterm-core`.

Product architecture is owned by `../architecture.md`. This document covers
the module structure, internal design decisions, and crate-level ADRs for
`scterm-core`.

## Module Responsibilities

The following is the expected module structure. Exact layout is authoritative
in `crates/scterm-core/src/`.

- `error` — `ScError` struct, kind classification, contextual accessors
- `session` — `SessionName`, `SessionPath`, validated constructors, and
  portable path rules after CLI/session expansion
- `ring` — in-memory ring buffer
- `packet` — client-to-master packet definitions and validator logic
- `state` — session master and attach client state types and consuming
  transitions
- `ancestry` — ancestry environment variable naming, chain parsing/rendering,
  and self-attach predicate
- `config` — `LogCap`, `RingSize`

## Dependency Rule

This crate has no dependencies outside the Rust standard library and
well-audited, platform-agnostic utilities approved in `../dependency-policy.md`.

See `../crate-boundaries.md` for the enforced dependency direction.

## ADR-TERM-CORE-001 — ScError as Struct, Not Enum

`ScError` is a struct with contextual fields and a private kind discriminant.
External callers use accessor methods and helper predicates rather than
exhaustive matching on error variants.

This prevents callers from coupling to internal error taxonomy and allows the
error surface to evolve without breaking API.

## ADR-TERM-CORE-002 — Typestate Consumes Old State

State transitions exposed across the public API boundary consume the old state
value and return the new state. Invalid transitions are unrepresentable at the
type level.

See REQ-RBP-003 in `../requirements.md`.

## ADR-TERM-CORE-003 — Domain Rules Stay Portable

The crate owns the portable rule set that higher layers consume, but not the
OS calls that discover runtime facts.

Examples:

- `scterm-core` owns stale-session classification as a domain condition, but
  not Unix socket I/O
- `scterm-core` owns ancestry parsing and self-attach detection, but not CLI
  exit-code rendering
- `scterm-core` owns log-cap and ring-size semantics, but not log file I/O

See `requirements.md` REQ-TERM-CORE-004 through REQ-TERM-CORE-006.
