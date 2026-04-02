# scterm Architecture

## Purpose

This document defines the target architecture for `scterm`.

Related documents:

- `requirements.md`
- `crate-boundaries.md`
- `dependency-policy.md`
- `compatibility-matrix.md`
- `protocol.md`
- `state-machines.md`
- `error-model.md`
- `testing-strategy.md`
- `public-api-checklist.md`
- `scterm-atm/bridge-spec.md` — ATM bridge injection contract
- `archive/implementation-plan.md` — archived phased delivery plan

Crate-level architecture documents:

- `scterm-core/architecture.md`
- `scterm-unix/architecture.md`
- `scterm-app/architecture.md`
- `scterm-atm/architecture.md`

The architecture is intentionally phased:

- Phase 1 reproduces the core `atch` design in Rust.
- Phase 2 adds ATM message injection as an adapter layered onto the same PTY
  session core.

## Architectural Principles

- Preserve raw PTY passthrough.
- Keep terminal compatibility by avoiding terminal emulation.
- Match `atch` behavior before extending it.
- Isolate platform-specific code behind narrow interfaces.
- Keep ATM outside the core session engine.
- Prefer explicit, testable flows over hidden magic.

## Boundary Rules

These boundaries are intended to stay stable even as the implementation grows.

### Core Session Boundary

The core session model owns:

- session identifiers and path rules;
- packet definitions and validation;
- scrollback and history policies;
- session ancestry rules;
- input injection ordering rules;
- user-visible session semantics.

The core session model must not depend on:

- the `atm` CLI;
- ATM-specific parsing or state;
- terminal UI libraries;
- platform-specific socket or PTY APIs beyond narrow traits.

### Platform Boundary

Platform code owns:

- PTY creation and child-process integration;
- Unix socket setup and cleanup;
- termios/raw-mode details;
- signal handling and resize propagation;
- daemonization mechanics.

Platform code must not define product semantics. It implements interfaces
required by the core and app layers.

### ATM Boundary

ATM code owns only:

- blocking reads from the external `atm` CLI;
- normalization of inbound message text;
- de-duplication state for delivered messages;
- conversion from inbound ATM events to sanitized injection requests.

ATM code must not:

- own the PTY;
- write directly to the PTY;
- decide session lifecycle policy;
- be required for non-ATM session use.

### CLI Boundary

CLI code owns:

- command parsing;
- compatibility aliases and legacy modes;
- rendering of user-facing success and error messages;
- wiring commands into master/client/app services.

CLI code must not become the place where business rules are duplicated. If a
rule matters outside parsing, it belongs below the CLI layer.

### Dependency Direction

Allowed dependency direction should be:

- binary -> app
- app -> core
- app -> platform
- app -> atm-adapter
- atm-adapter -> core
- platform -> core

Forbidden dependency direction should be:

- core -> platform
- core -> atm-adapter
- platform -> atm-adapter
- atm-adapter -> platform internals
- CLI directly orchestrating PTY/socket details without going through app

## System Overview

At a high level, `scterm` has the same shape as `atch`:

- a CLI entry point that resolves commands and session paths;
- a master process that owns the PTY, child process, socket, scrollback, and
  log;
- an attach client that connects to the master and binds the user terminal to
  the session; and
- persistent session storage on the local filesystem.

Phase 2 adds:

- an optional ATM watcher adapter that receives inbound ATM messages through the
  external `atm` CLI and forwards them into the master as injection events.

## High-Level Component Model

```text
user terminal
    |
    v
attach client <---- local control socket ----> master daemon ----> PTY ----> child program
    |                                             |    |   |
    |                                             |    |   +-- session log
    |                                             |    +------ scrollback ring
    |                                             +----------- ATM watcher adapter (optional)
    |
    +-- raw terminal mode, input handling, log replay
```

## Recommended Crate Topology

The workspace should stay small. The goal is clean boundaries, not crate
proliferation.

### Recommended Starting Point

Use 3 crates for sprint 1, then add a 4th optional crate when ATM lands.

### Crate 1: `scterm-core`

`scterm-core` owns the portable domain model: validated session identifiers,
ancestry rules, packet and typestate contracts, history primitives, and typed
domain errors.

See:

- `scterm-core/requirements.md`
- `scterm-core/architecture.md`

### Crate 2: `scterm-unix`

`scterm-unix` owns Unix runtime primitives only: PTY/socket transport,
raw-mode, signals, daemonization, and Unix-specific filesystem operations such
as long-path socket support.

See:

- `scterm-unix/requirements.md`
- `scterm-unix/architecture.md`

### Crate 3: `scterm-app`

`scterm-app` owns the product choreography: command dispatch, master/client
orchestration, PTY-input serialization, attach/replay ordering, user-facing
messages, exit-code mapping, and the sc-observability-backed `AppLogger`.

See:

- `scterm-app/requirements.md`
- `scterm-app/architecture.md`

### Crate 4: `scterm-atm` (Phase 2)

`scterm-atm` owns the optional ATM adapter: blocking CLI reads, relevance
filtering, normalization, dedupe, and conversion into normalized injection
requests for the app layer.

See:

- `scterm-atm/requirements.md`
- `scterm-atm/architecture.md`
- `scterm-atm/bridge-spec.md`

### Binary Crate

The top-level binary crate should stay very thin:

- parse process startup context;
- construct the app services;
- execute the selected command path;
- map errors to exit codes and stderr messages.

If preferred, this binary can live inside `scterm-app` as `src/bin/scterm.rs`
rather than as a separate workspace member. That keeps the effective crate
count at 3 in sprint 1.

## Why Not More Crates

More than 4 workspace crates is probably premature for this project.

Splitting out separate crates for CLI, ring buffer, protocol, or logging would
increase coordination cost without buying meaningful isolation yet. The main
pressure lines are:

- product semantics vs. platform details;
- core semantics vs. ATM integration;
- binary shell vs. reusable orchestration.

Those are the boundaries worth protecting first.

## Runtime Boundary

The runtime model must remain explicit.

- `scterm-core` is runtime-agnostic and contains no Tokio types.
- long-lived async I/O belongs in `scterm-unix`, `scterm-app`, and later
  `scterm-atm`;
- async runtime choices must not leak into domain types or path-validation
  logic;
- the core should remain unit-testable without a reactor or PTY.

## Structured Logging Boundary

Structured logging is intentionally application-owned. The top-level rule is
that only `scterm-app` and the final binary configure or own the logger
lifecycle; lower crates remain logging-implementation-agnostic.
`scterm-app` uses `sc-observability` as the mandated backend per
`scterm-app/architecture.md` (`ADR-TERM-APP-006`).

The crate-local logging design lives in:

- `scterm-app/requirements.md`
- `scterm-app/architecture.md`

## Phase 1 Core Components

Phase 1 is still organized around four system components:

- CLI layer
- master daemon
- attach client
- session storage

Their crate-local ownership and detailed responsibilities now live in:

- `scterm-app/architecture.md` — CLI layer, master daemon, attach client, and
  storage coordination
- `scterm-unix/architecture.md` — PTY/socket/raw-mode/signal/daemonization
  runtime primitives
- `scterm-core/architecture.md` — packet/state/history/ancestry domain models

Top-level architecture keeps only the cross-cutting system contract below.

## Control and Data Planes

### Client to Master

Client-to-master traffic is structured control/data packets.

#### Wire Format

The normative packet layout and per-type semantics are owned by
`scterm-core/architecture.md` under `Control Packet Contract`.

At the system level, the architecture only requires that client-to-master
traffic remain a fixed-size binary control packet compatible with `atch`.

### Master to Client

Master-to-client traffic remains raw PTY output bytes, not a higher-level
terminal protocol.

This is a critical compatibility choice. It preserves the `atch` property that
the master does not reinterpret the terminal stream.

## Session Lifecycle Flows

### Create and Attach

1. CLI resolves the session path and child command.
2. Master creates, binds, and listens on the control socket, then opens the
   persistent log.
3. Master spawns the PTY child.
4. Attach client replays the on-disk log (read directly from filesystem).
5. Attach client connects to the session socket.
6. Client sends `attach` and `redraw`.
7. Master optionally replays the ring buffer.
8. Session switches to steady-state streaming.

### Attach Existing Session

1. Client resolves the session path.
2. Client replays the on-disk log if present (direct filesystem read).
3. Client connects to the session socket and sends `attach`.
4. Master replays ring history if needed.
5. Live PTY output resumes.

### Startup Readiness

Startup readiness is normatively defined in `requirements.md` under
`Session Lifecycle`.

Architecturally, readiness is enforced by `scterm-app` orchestration over
`scterm-unix` socket and PTY runtime primitives.

### Stale Socket Definition

Stale socket classification is normatively defined in `requirements.md` under
`Stale Socket Definition`.

Architecturally, stale detection still preserves the log file and session
directory so history replay remains structurally possible after stale recovery
begins.

### Stale Session Recovery

1. CLI or client attempts `connect()` on the session socket.
2. `connect()` returns `ECONNREFUSED` → socket is stale.
3. Client replays the on-disk log if present (history is still valid).
4. Default open mode removes the stale socket file and creates a fresh session.
5. `attach` command fails with a clear error indicating a stale session was
   found and suggests using default open mode to recover.

### Detach

1. Client detects detach key.
2. Client exits while restoring its local terminal.
3. Master remains alive with the PTY child and session state intact.

## PTY Ownership Model

The PTY is exclusively owned by the master daemon.

All writes into the child process stdin path must pass through the master:

- normal attached-client keystrokes;
- `push` command traffic;
- redraw-triggering control writes such as `Ctrl-L`;
- future ATM-driven message injections.

This serialization point is mandatory. It keeps ordering deterministic and
prevents races between human input and synthetic input.

The master also preserves `atch`'s wait-for-first-attach behavior: PTY output
produced before the first attached client is retained in the persistent log and
ring buffer, but it is not broadcast live to zero clients. The first client
receives that history through replay, then joins the live stream.

## History Model

`scterm` keeps two complementary history stores.

### Ring Buffer

- in-memory only;
- bounded, fixed-size;
- optimized for fast catch-up on active sessions;
- lost when the master exits.

### Persistent Log

- plaintext file on disk;
- survives detach, child exit, crashes, and reboot;
- replayed before live attach;
- capped to a configured maximum size.

The client may skip ring replay after log replay when the same history would be
duplicated.

## Environment and Session Identity

Each child process receives a derived session ancestry environment variable such
as `SCTERM_SESSION`.

The value is a colon-separated chain of session socket paths. This is used for:

- self-attach prevention;
- nested session detection;
- `current` command output.

The environment variable name is derived from the binary basename so renamed
executables keep the same semantics as `atch`.

## Platform Boundaries

Phase 1 targets Unix-like systems, specifically macOS and Linux.

Platform-specific concerns include:

- PTY creation and resizing;
- Unix domain sockets;
- termios raw-mode handling;
- signal forwarding and process groups;
- daemonization behavior.

The business logic for session lifecycle, history, option semantics, and input
ordering should remain platform-agnostic wherever practical.

## ATM Integration Architecture

Phase 2 adds ATM without making ATM part of the core session runtime model.

### Design Rule

ATM is an adapter, not a foundational dependency.

That means:

- no ATM crates in `Cargo.toml`;
- no direct ATM Rust imports;
- no `ATM_HOME` assumptions;
- no requirement that ATM exist for core terminal use.

Structured logging remains app-owned, and `scterm-app` uses
`sc-observability` as the mandated backend per `scterm-app/architecture.md`
(`ADR-TERM-APP-006`).

### ATM Watcher Adapter

The ATM bridge is a separate watcher component launched only when ATM
integration is enabled.

Responsibilities:

- invoke the external `atm` CLI to block for inbound messages;
- filter inbound messages to those relevant to the current session;
- parse sender identity and body text from CLI output;
- de-duplicate messages using local state;
- emit normalized inbound message events to the master.

The watcher should use blocking CLI behavior such as `atm read --timeout 600`
or an equivalent ATM subscription mode rather than implementing its own busy
polling loop.

The watcher should communicate with the app layer using typed events rather
than shell text blobs once parsing is complete.

### ATM Relevance Filter

The normative relevance criteria are defined in `requirements.md` under
`ATM Message Relevance Filter`.

Architecturally, the filter is owned by `scterm-atm` and evaluated before the
app layer sees an inbound injection request.

## ATM Injection Flow

The architectural requirement is to inject inbound ATM messages into the active
session in a way that wakes agent terminals that are otherwise idle at the end
of a turn.

### Injection Sequence

1. ATM watcher receives a new inbound message.
2. Watcher normalizes it into an `InboundMessage` event.
3. Master serializes that event against all other pending PTY writes.
4. Master writes synthesized input into the PTY for the child process.
5. The synthesized input ends with carriage return.
6. The child process wakes and processes the new input as if it had been
   entered locally.

### Why Input Injection

The goal is not merely to display a notification to human observers.

The goal is to wake agent-oriented terminal programs such as Codex or Gemini
that may otherwise sit idle waiting for local user input after a turn boundary.
That requires PTY input injection, not only PTY output decoration.

### Message Envelope

The exact injected payload format, sanitization rules, and final
carriage-return behavior are normatively defined in `requirements.md`
(`Message Format`) and `scterm-atm/bridge-spec.md`.

Architecturally, `scterm-atm` produces normalized injection requests and
`scterm-app` serializes them onto the master-owned PTY input path.

## Failure Containment

ATM failures must not take down the session core.

Specifically:

- if `atm` is not installed, session management still works;
- if the watcher crashes, the master and PTY continue running;
- if a message cannot be parsed, the watcher may drop that message but must not
  corrupt the PTY stream;
- duplicate suppression state must be local to `scterm`, not delegated to ATM
  internals.

## Boundary Tests

Architecture should be enforced with tests and review rules, not only prose.

Recommended checks:

- compile-fail or lint checks that `scterm-core` has no ATM or Unix-runtime
  dependencies;
- integration tests that drive `scterm-app` through mocked runtime traits;
- Unix-only integration tests in `scterm-unix`;
- feature-gated ATM tests that verify the app still builds and runs when the
  ATM crate is disabled.

## Documentation Boundary

Public Rust APIs are part of the architecture and must be treated as such.

- every public crate gets crate-level docs;
- every public module gets `//!` docs;
- public functions and types follow canonical doc-section conventions;
- major session flows should have doctest-style or example-based coverage;
- internal re-exports meant to be part of the main API use `#[doc(inline)]`.

## Security Considerations

### Core Session Security

- sockets and logs are owner-only files;
- attached clients are trusted by virtue of local filesystem access;
- session logs may contain secrets and must be treated as sensitive plaintext.

### ATM Bridge Security

- inbound ATM content is untrusted text;
- control characters must be sanitized before injection;
- the bridge must not execute message content as shell code;
- the bridge must not grant more access than the local user already has.

## Testing Strategy

### Compatibility Layer

Phase 1 testing should begin by porting the `atch/tests/test.sh` behaviors into
Rust integration tests.

These tests should verify:

- CLI dispatch and aliases;
- legacy compatibility modes;
- session path resolution;
- attach/create semantics;
- push, kill, clear, list, and current;
- log persistence and stale-session behavior;
- non-TTY error handling.

### ATM Extension Tests

Phase 2 tests should verify:

- watcher isolation from the core session engine;
- exactly-once ATM injection per message;
- ordered coexistence of user input and ATM injection;
- carriage-return nudge behavior;
- graceful degradation when `atm` is unavailable.

## Rust Design Requirements

These are mandatory design constraints established by pre-sprint RBP review.
All four must be in place before Sprint 1 code lands.

### RBP-1 (Blocking) — Library and Application Error Boundary

`scterm-core` exports a typed library error, while the app layer owns the final
application error boundary.

Crate-local ownership:

- `scterm-core/requirements.md` — `ScError` contract
- `scterm-app/requirements.md` — application error boundary and exit mapping
- `scterm-atm/requirements.md` — ATM adapter failures outside core

### RBP-2 (Blocking) — Coarse Typestate Lifecycle

Use typestate where it removes invalid public transitions without turning every
internal phase into type noise.

The normative state set and transition rules remain product architecture, but
their crate-local home is `scterm-core`.

See:

- `scterm-core/requirements.md` (`REQ-TERM-CORE-005`)
- `scterm-core/architecture.md`
- `state-machines.md`

### RBP-3 (Important) — Sealed Platform Traits

All platform abstraction traits remain sealed from day one. The concrete trait
boundary lives in `scterm-unix`.

See:

- `scterm-unix/requirements.md` (`REQ-TERM-UNIX-001`)
- `scterm-unix/architecture.md`

### RBP-4 (Important) — Domain Newtypes and Builder Shapes

Validated domain newtypes and builder-shape rules are owned by
`scterm-core`.

See:

- `scterm-core/requirements.md` (`REQ-TERM-CORE-003`)
- `scterm-core/architecture.md`

### RBP-5 (Blocking) — Documentation and API Hygiene

Public Rust APIs are contractual surface area.

This remains a workspace-wide rule. Detailed crate-level checklists live in
`public-api-checklist.md` and the per-crate requirement docs.

### RBP-6 (Blocking) — Lints and Tooling Gates

The workspace should enable linting policy before feature work starts.

This remains a workspace-wide rule; the top-level policy is defined here and
the detailed ownership is tracked through the per-crate docs and
`public-api-checklist.md`.

### RBP-7 (Blocking) — Unsafe Containment

Unsafe code is a platform implementation detail, not a general-purpose escape
hatch.

The concrete unsafe-bearing boundary is owned by `scterm-unix`.

See:

- `scterm-unix/requirements.md` (`REQ-TERM-UNIX-002`)
- `scterm-unix/architecture.md`

## Architectural Summary

`scterm` should deliberately keep the elegant `atch` shape:

- one master per session;
- one PTY owned by that master;
- raw output broadcast to clients;
- explicit packetized control from clients to master;
- durable history via log plus ring buffer.

The ATM extension should not change that model. It should feed one additional
input source into the master: normalized inbound ATM messages, serialized onto
the same PTY write path as every other input source.
