# scterm-app Architecture

## Purpose

This document defines the crate-local architecture of `scterm-app`.

Product architecture is owned by `../architecture.md`. This document covers
the module structure, internal design decisions, and crate-level ADRs for
`scterm-app`.

## Module Responsibilities

The following is the expected module structure. Exact layout is authoritative
in `scterm-app/src/` and `src/` (binary).

- `master` — master loop, PTY ownership, socket server, client dispatch
- `client` — attach client loop, log replay, ring replay, live streaming
- `cli` — command parser, aliases, legacy mode handling
- `commands` — per-command dispatch and wiring
- `messages` — user-facing message rendering
- `logging` — structured logging initialization and `AppLogger` lifecycle
- `exit` — exit code mapping from `ScError` and app failures

## Runtime Components

### CLI Layer

The CLI-facing portion of `scterm-app` owns:

- command-based and legacy-compatible invocation forms
- option precedence and placement rules
- command selection for attach, create, list, push, kill, clear, and current
- translation from parsed CLI input into app-layer session workflows

### Master Daemon

The master-side portion of `scterm-app` owns:

- control-socket server orchestration
- PTY child supervision
- client packet dispatch
- PTY output fan-out to attached clients
- session log and ring-buffer orchestration
- the single serialized PTY-input path
- attached-state metadata management

The master is the only component that may write to the PTY file descriptor.
Conceptually, every PTY-bound source flows through one app-owned queue such as:

- `enqueue_user_input`
- `enqueue_push_input`
- `enqueue_redraw_input`
- `enqueue_inbound_message`

Output observation is deferred from Sprint 1. A future hook point may exist
only at the app layer, post-PTY-read and pre-broadcast, and must remain
observe-only, non-blocking, and unable to mutate or backpressure log or client
broadcast behavior.

### Attach Client

The attach-side portion of `scterm-app` owns:

- on-disk log replay before live attach
- socket connect and ring-replay orchestration
- stdin forwarding through the master-owned PTY-input path
- detach and suspend behavior
- terminal restoration on all exit paths

### Session Storage Coordination

`scterm-app` coordinates three session storage surfaces:

- the control socket
- the persistent plaintext log
- the in-memory ring buffer

The storage primitives themselves belong to lower layers, but the app layer
decides when they are replayed, truncated, appended to, or surfaced to the
user.

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

The sc-observability-backed `AppLogger` is wired at the binary entry point by
`scterm-app`. No lower crate initializes or shuts down the logging subsystem.

## ADR-TERM-APP-003 — anyhow at the Application Boundary Only

`anyhow` is used inside `scterm-app` and the binary. It is not re-exported and
is not used in `scterm-core` or `scterm-unix`.

## ADR-TERM-APP-004 — App Layer Owns Session Choreography

This crate is where the product's session choreography lives.

It owns:

- detached startup readiness
- attach ordering across log replay, socket connect, ring replay, and live
  mode
- the wait-for-first-attach rule
- multi-client disconnect ordering and attached-state metadata
- final mapping of typed errors into UX and exit status

It does not own PTY/socket primitives or portable domain predicates.

## ADR-TERM-APP-005 — Self-Contained AppLogger (SUPERSEDED by ADR-TERM-APP-006)

~~This crate standardizes on a self-contained `AppLogger` implemented with
`serde_json` and `std::io` instead of a dependency on the sibling
`sc-observability` workspace.~~

**Superseded.** This decision was reversed by ADR-TERM-APP-006 when the
ecosystem consistency requirement became clear: logs from `scterm` and `schook`
must land in a consistent location when both are loaded in the ATM application.

## ADR-TERM-APP-006 — AppLogger Backed by sc-observability

`scterm-app` uses `sc-observability` as the structured logging backend.

Rationale:

- When `scterm` and `schook` are both used in the ATM application, all tool
  logs must go to a consistent root directory so they are discoverable together.
  A self-contained `AppLogger` writing to its own root provides no coordination
  with the rest of the ecosystem.
- `sc-observability` provides `LoggerConfig::default_for(service, log_root)`,
  where the log root is injected by the caller. That keeps shared log-root
  selection outside `scterm-app` while preserving the repo's ATM boundary rule.
- `sc-observability` provides a path toward OTel export (`sc-observability-otlp`)
  when that requirement matures, without requiring another breaking change to
  this crate's logging integration point.

Architecture:

- `scterm-app` depends on `sc-observability` and `sc-observability-types`.
- `scterm-core` and `scterm-unix` remain logging-implementation-agnostic.
- `AppLogger` in `scterm-app/src/logging.rs` wraps `sc_observability::Logger`
  behind the same `emit(target, action, message) -> Result<()>` API —
  callers are unchanged.

Dependency constraint:

- A path dep to the local
  `../../../../sc-observability/crates/sc-observability` workspace path is
  used until the crate is published to crates.io as part of the versioning
  standards rollout, at which point the path dep is replaced with a version
  pin.
