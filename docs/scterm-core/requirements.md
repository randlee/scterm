# scterm-core Requirements

## Purpose

This document defines what `scterm-core` owns and how it satisfies referenced
product requirements.

Product requirements are owned by `../requirements.md`. This document must not
restate the product contract — it references it.

## Requirement ID Namespace

Crate-level requirements use the prefix `REQ-TERM-CORE-*`.
Crate-level architecture decisions use the prefix `ADR-TERM-CORE-*`.

## What This Crate Owns

`scterm-core` owns the domain model and all portable session semantics:

- session identifier and path resolution rules
- domain newtypes: `SessionName`, `SessionPath`, `LogCap`, `RingSize`
- session ancestry helpers
- client-to-master packet model
- ring buffer
- session state machine types (`Resolved`, `Running`, `Stale`)
- attach client state machine types (`LogReplaying`, `Connecting`,
  `RingReplaying`, `Live`, `Detached`)
- self-attach prevention predicate
- domain error type (`ScError`)

## What This Crate Must Not Own

- PTY or socket APIs
- platform-specific OS primitives
- ATM-specific parsing or state
- terminal UI libraries
- application-level structured logging configuration

## Product Requirements Satisfied

The following product requirements from `../requirements.md` are implemented
by this crate:

- Session Model section — path resolution, session directory rules, socket
  path depth handling
- REQ-RBP-001 — library error policy: `ScError` struct, typed library errors
- REQ-RBP-003 — typestate at coarse lifecycle boundaries: session master and
  attach client states
- REQ-RBP-005 — domain newtypes: `SessionName`, `SessionPath`, `LogCap`,
  `RingSize`
- Self-attach prevention predicate (Environment and Nesting section)
- Ring buffer and session history policies (Session History section)

## REQ-TERM-CORE-001 — No Platform or ATM Dependencies

`scterm-core` shall compile without Tokio, PTY, Unix socket, or ATM-specific
dependencies.

Satisfies: Core Principles in `../requirements.md`.

## REQ-TERM-CORE-002 — Public API Returns ScError

All public API functions in this crate shall return `Result<T, ScError>`.

Satisfies: REQ-RBP-001 in `../requirements.md`.

## REQ-TERM-CORE-003 — Newtype Constructors Return Result

All newtype constructors (`SessionName::new`, `SessionPath::new`, `LogCap::new`,
`RingSize::new`) shall return `Result<T, ScError>`.

Satisfies: REQ-RBP-005 in `../requirements.md`.
