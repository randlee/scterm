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
- `session` — `SessionName`, `SessionPath`, newtype constructors, path resolution
- `ring` — in-memory ring buffer
- `packet` — client-to-master packet definitions
- `state` — session master and attach client state types
- `ancestry` — ancestry environment variable handling, self-attach predicate
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
