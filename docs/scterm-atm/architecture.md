# scterm-atm Architecture

## Purpose

This document defines the crate-local architecture of `scterm-atm`.

Product architecture is owned by `../architecture.md`. This document covers
the module structure, internal design decisions, and crate-level ADRs for
`scterm-atm`.

## Module Responsibilities

The following is the expected module structure. Exact layout is authoritative
in `scterm-atm/src/`.

- `watcher` — blocking `atm` CLI reader loop, reconnect policy
- `filter` — inbound message relevance predicate
- `normalize` — message text sanitization, typed event construction
- `dedupe` — per-session delivery tracking, exactly-once enforcement
- `inject` — serialized injection request construction (does not write PTY)

The actual PTY write remains in `scterm-app`. This crate produces injection
requests; the app layer decides when and how to write them.

## Dependency Direction

`scterm-atm` depends on `scterm-core` for domain types.
It does not depend on `scterm-unix` or `scterm-app`.

See `../crate-boundaries.md` for the enforced dependency direction.

## Integration Contract

See `bridge-spec.md` for the full injection contract, sanitization rules,
de-duplication rules, and failure policy.

## ADR-TERM-ATM-001 — External CLI, Not Rust Crate

The `atm` integration uses the external `atm` CLI via a blocking subprocess
read, not ATM Rust crates. This preserves the ATM boundary and keeps
`scterm-atm` independently deployable.

## ADR-TERM-ATM-002 — Watcher Failure Does Not Kill the Master

ATM watcher failures are isolated. The master process and PTY child remain
alive when the watcher errors or exits unexpectedly. Recovery is the
watcher's responsibility.

## ADR-TERM-ATM-003 — Normalize Before Injection

The ATM adapter owns normalization before the app layer sees a message.

That includes:

- sender identity shaping
- text sanitization
- typed event construction
- exactly-once dedupe decisions

`scterm-app` receives a normalized injection request, not raw ATM CLI text.
