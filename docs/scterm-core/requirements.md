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

- validated session identifiers and session-path newtypes
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

- Session Model section — validated session identifiers and path rules after
  the app layer resolves user-visible defaults
- REQ-RBP-001 — library error policy: `ScError` struct, typed library errors
- REQ-RBP-003 — typestate at coarse lifecycle boundaries: session master and
  attach client states
- REQ-RBP-005 — domain newtypes: `SessionName`, `SessionPath`, `LogCap`,
  `RingSize`
- Environment and Nesting section — ancestry derivation, `current` rendering,
  `clear` default-target derivation, self-attach predicate
- Session History section — ring buffer rules, replay-order contract, and
  default history-cap newtypes
- Stale Socket Definition section — stale-session classification predicate
- Exit Codes section — typed error conditions consumed by the app-layer mapper

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

Configuration or constructor paths requiring four or more semantically
distinct parameters shall use a builder or grouped config type rather than a
long positional parameter list.

Satisfies: REQ-RBP-005 in `../requirements.md`.

## REQ-TERM-CORE-004 — Ancestry and Self-Attach Semantics

`scterm-core` shall own the normative ancestry contract:

- derive the ancestry environment variable name from the executable basename
- transform that basename by uppercasing it, replacing non-alphanumeric
  characters with `_`, and suffixing `_SESSION`
- render and parse the colon-delimited socket-path chain
- append nested ancestry as `previous_chain + ":" + current_socket`
- treat a non-nested session as a single socket path with no colon
- impose no fixed nesting-depth cap in Sprint 1
- derive the `current` display chain from ancestry segments
- derive the innermost default target used by `clear` when no explicit session
  is provided
- detect self-attach loops by exact full-path comparison before any socket
  connection attempt
- perform self-attach prevention in this order:
  1. expand the target session path from the argument or default rules
  2. scan each colon-delimited ancestry segment for exact full-path equality
     with the target socket path
  3. if any segment matches, return a typed self-attach `ScError`
  4. perform the check before any socket connect attempt

Satisfies: Environment and Nesting section and Exit Codes section in
`../requirements.md`.

## REQ-TERM-CORE-005 — Packet and Typestate Contracts

`scterm-core` shall own the portable control-packet model and the coarse
typestate model used by the session master and attach client.

- packet type validation, selector-byte validation, and fixed-payload
  invariants belong here
- `Resolved`, `Running`, and `Stale` are the public master-side coarse states
- `LogReplaying`, `Connecting`, `RingReplaying`, `Live`, and `Detached` are
  the public attach-side coarse states
- stale-session classification is represented as a typed domain condition for
  the caller to map into UX and exit status

Satisfies: Session Lifecycle section, Stale Socket Definition section, and
REQ-RBP-003 in `../requirements.md`.

## REQ-TERM-CORE-006 — Portable History Primitives

`scterm-core` shall own the portable history primitives shared across app and
platform layers:

- ring-buffer capacity and truncation rules
- log-cap parsing and disabled-log representation
- replay-order contract: on-disk log before ring replay, with ring replay
  skippable when disk history already covers the same bytes

The actual log file I/O and replay orchestration do not belong here.

Satisfies: Session History section in `../requirements.md`.

## REQ-TERM-CORE-007 — No Unsafe in scterm-core

`scterm-core` shall contain no `unsafe` blocks in Sprint 1.

Satisfies: REQ-RBP-007 in `../requirements.md`.
