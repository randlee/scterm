# scterm-app Requirements

## Purpose

This document defines what `scterm-app` owns and how it satisfies referenced
product requirements.

Product requirements are owned by `../requirements.md`. This document must not
restate the product contract — it references it.

## Requirement ID Namespace

Crate-level requirements use the prefix `REQ-TERM-APP-*`.
Crate-level architecture decisions use the prefix `ADR-TERM-APP-*`.

## What This Crate Owns

`scterm-app` owns session orchestration and CLI integration:

- master loop (PTY owner, socket server)
- attach client loop (log replay, ring replay, live streaming)
- PTY input serialization across all input sources
- session log replay and ring replay orchestration
- structured logging setup and logger lifecycle via the
  `sc-observability`-backed `AppLogger`
- CLI command parsing, aliases, legacy single-letter modes
- user-facing message rendering and exit codes
- wiring of platform services and domain logic into session workflows

## What This Crate Must Not Own

- PTY or socket primitives (owned by `scterm-unix`)
- Domain newtypes and session state machines (owned by `scterm-core`)
- ATM-specific parsing or watcher logic (owned by `scterm-atm`)
- Business rules that must apply outside the CLI layer

## Product Requirements Satisfied

The following product requirements from `../requirements.md` are implemented
by this crate:

- Session Lifecycle section — orchestration of master and attach client loops
- Commands and Command Semantics sections — full command surface
- Option Handling section — all CLI options and placement rules
- Structured Logging section — `AppLogger` wiring and logger lifecycle
- Multi-Client Detach and Kill Semantics section — orchestration of client
  disconnects on kill
- Error Handling and UX section — user-facing messages and exit codes
- Exit Codes table — mapping from `ScError` and ATM failure modes to codes
- REQ-RBP-001 — application error boundary: `anyhow` is the app-layer strategy
- REQ-RBP-002 — public documentation standard: all public surface documented
- REQ-RBP-006 — lints and tooling from day one

## REQ-TERM-APP-001 — Single Application Error Strategy

`scterm-app` and the final binary shall use exactly one application-level error
strategy (`anyhow`). The library crates `scterm-core` and `scterm-unix` must
not be wrapped by a second error strategy.

Satisfies: REQ-RBP-001 in `../requirements.md`.

## REQ-TERM-APP-002 — CLI Rules Do Not Duplicate Core Rules

If a rule matters outside argument parsing, it belongs in `scterm-core`, not
in the CLI layer.

Satisfies: CLI Boundary section in `../architecture.md`.

## REQ-TERM-APP-003 — Logger Lifecycle Is App-Layer Only

`scterm-app` owns logger initialization and shutdown. Lower crates
(`scterm-core`, `scterm-unix`) must not configure sinks, logger lifecycle, or
log file policy.

The decision to standardize on an `sc-observability`-backed `AppLogger` is
recorded in `architecture.md` (`ADR-TERM-APP-006`). `scterm-app` is the only
crate that owns the logging backend choice; lower crates remain backend
agnostic as required by `../requirements.md` (`Structured Logging`) and
`../dependency-policy.md` (`Observability Dependency Policy`).

Satisfies: Structured Logging section in `../requirements.md`.

## REQ-TERM-APP-004 — Command Surface and Compatibility Wiring

`scterm-app` shall own the complete command surface after parsing:

- verb commands, aliases, and legacy single-letter modes
- default-open orchestration and `attach`/`new`/`start`/`run` behavior
- option placement compatibility and `--` handling
- `list`, `current`, `push`, `kill`, and `clear` dispatch

This crate consumes domain predicates from `scterm-core`; it must not
re-implement those predicates locally.

Satisfies: Commands section, Command Semantics section, and Option Handling
section in `../requirements.md`.

## REQ-TERM-APP-005 — Startup, Attach, and Replay Orchestration

`scterm-app` shall own the orchestration rules that turn domain and platform
primitives into a running session:

- detached startup readiness and timeout policy
- wait-for-first-attach behavior
- attach order: log replay, socket connect, ring replay, then live streaming
- attached-state metadata management for `list`
- multi-client disconnect ordering on detach, kill, and child exit

Satisfies: Session Lifecycle section, Session History section, and Multi-Client
Detach and Kill Semantics section in `../requirements.md`.

## REQ-TERM-APP-006 — Single PTY Write Path

All PTY-bound input sources shall be serialized through a single app-owned
write path in the master loop.

- attached-client stdin
- `push`
- redraw-triggering control writes
- ATM inbound message injections supplied by `scterm-atm`

No client, adapter, or lower crate may write directly to the PTY file
descriptor.

Satisfies: PTY and Client Behavior section in `../requirements.md` and
ADR-TERM-APP-001 in `architecture.md`.

## REQ-TERM-APP-007 — UX and Exit-Code Ownership

`scterm-app` and the final binary shall own:

- user-facing error text
- quiet-mode and empty-`list` rendering
- mapping typed domain/runtime failures into the stable exit-code table

Satisfies: Environment and Nesting section, Error Handling and UX section,
Exit Codes section, and REQ-RBP-001 in `../requirements.md`.

## REQ-TERM-APP-008 — No Unsafe in scterm-app

`scterm-app` and any binary targets it owns shall contain no `unsafe` blocks in
Sprint 1.

Satisfies: REQ-RBP-007 in `../requirements.md`.
