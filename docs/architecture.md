# scterm Architecture

## Purpose

This document defines the target architecture for `scterm`.

Related documents:

- `requirements.md`
- `crate-boundaries.md`
- `dependency-policy.md`
- `implementation-plan.md`
- `compatibility-matrix.md`
- `protocol.md`
- `state-machines.md`
- `error-model.md`
- `testing-strategy.md`
- `atm-bridge-spec.md`
- `public-api-checklist.md`

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

Owns:

- packet types;
- session config and option semantics after parsing;
- session path resolution rules;
- session ancestry/env-var derivation;
- ring buffer implementation;
- ATM-independent synthesized-input request types, only if a shared domain type
  is needed at all;
- shared error and result types that are independent of concrete I/O backends.

Must not depend on:

- Tokio
- `portable-pty`
- Unix socket types
- `atm`

Public API expectations:

- public types use strong domain newtypes instead of raw primitives;
- public I/O helpers prefer standard traits such as `Read`, `Write`, `AsRef`,
  and `RangeBounds` where that improves ergonomics;
- public modules and items are documented to Rust API guideline standards.

### Crate 2: `scterm-unix`

Owns:

- Unix domain socket transport;
- PTY integration;
- raw terminal mode handling;
- signal handling;
- process-group and daemonization mechanics;
- filesystem primitives that are platform-specific.

This crate is the Unix implementation of the runtime-facing interfaces used by
the app layer.

### Crate 3: `scterm-app`

Owns:

- master/session orchestration;
- attach-client orchestration;
- command dispatch;
- structured logging configuration and emission;
- ordering of PTY writes from human input, `push`, redraw, and future ATM
  injections;
- compatibility behavior derived from `atch`.

This is where the product behavior lives. It depends on `scterm-core` plus the
selected runtime/platform implementation.

Application error policy:

- `scterm-app` may use `anyhow` at the orchestration boundary;
- typed library errors from `scterm-core` and `scterm-unix` are preserved until
  that boundary;
- the binary maps the final application error into exit codes and user-facing
  output.

Structured logging policy:

- use the self-contained `AppLogger` in `scterm-app` (serde_json + std::io) — no external observability crate dependency
- keep logger lifecycle and sink configuration in `scterm-app` or the binary, not in lower crates

### Crate 4: `scterm-atm` (Phase 2)

Owns:

- integration with the external `atm` CLI;
- blocking read loop or subscription handling;
- inbound message normalization;
- delivery de-duplication;
- conversion to `InboundMessage` events for `scterm-app`.

This crate should remain optional and feature-gated.

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

Structured logging uses a self-contained `AppLogger` implemented directly in
`scterm-app` using `serde_json` and `std::io`. No external observability crate
dependency is required or permitted in this repo.

Boundary rules:

- only `scterm-app` and the final binary own and configure the `AppLogger`
- `scterm-core` stays logging-implementation-agnostic
- `scterm-unix` stays logging-implementation-agnostic
- lower crates prefer rich typed errors and return values over ad-hoc logging

This keeps local structured logs available immediately without widening the
core architecture to include broader observability concerns.

## Phase 1 Core Components

### CLI Layer

Responsibilities:

- parse command-based and legacy-compatible invocation forms;
- resolve session names into socket and log paths;
- apply option precedence and placement rules;
- select the command path: attach, create, list, push, kill, clear, current.

The CLI layer is policy-heavy but state-light. It should not own PTY or socket
logic.

### Master Daemon

Responsibilities:

- create and own the control socket;
- spawn and supervise the PTY child process;
- receive client packets;
- forward PTY output to attached clients;
- maintain the in-memory scrollback ring;
- append output to the persistent log;
- manage session cleanup and end markers;
- arbitrate all writes into the PTY input stream.

The master is the single source of truth for a session.

**Attached-state metadata**: The master maintains a per-session attached-state
flag equivalent to `atch`'s socket execute-bit marker. The master sets this
flag when the first client attaches and clears it when the last client
detaches. This flag is readable by `list` to distinguish running-with-clients
from running-without-clients. The flag is master-owned; no client or adapter
may toggle it directly.

The master should expose one explicit serialized input path, conceptually:

- `enqueue_user_input`
- `enqueue_push_input`
- `enqueue_redraw_input`
- `enqueue_inbound_message`

Architecturally, all input sources flow through one master-owned PTY write
path. The implementation may use a channel, a loop-owned queue, or any other
single-point arbiter, but no clients, adapters, or lower layers may write
directly to the PTY file descriptor. Single ownership of the write path is the
rule; the exact serialization mechanism is an implementation detail owned by
`scterm-app`.

Output observation (tool-call tap) is deferred from Sprint 1. A hook point may
be reserved only at the app layer, at the post-PTY-read / pre-broadcast tee
point in the master read loop. It must be observe-only, non-blocking, and must
not mutate or backpressure the PTY stream, the persistent log, or client
broadcast.

### Attach Client

Responsibilities:

- connect to the session socket;
- replay the on-disk log before live attach;
- request or skip ring replay as appropriate;
- place the local terminal into raw mode;
- forward stdin to the master;
- detect detach and suspend behavior;
- forward resize events;
- restore the original terminal state on exit.

### Session Storage

Per session:

- socket file for local client/master coordination;
- plaintext log file for persistent history;
- in-memory ring buffer for low-latency scrollback replay.

## Control and Data Planes

### Client to Master

Client-to-master traffic is structured control/data packets.

#### Wire Format

All client-to-master packets share a common fixed layout compatible with
`atch`:

```
Offset  Size   Field
------  -----  -----
0       1      packet type (u8, see table below)
1       1      length / selector byte (u8)
2       8      fixed payload area (`sizeof(struct winsize)` on the reference platform)
```

This is a 2-byte header plus fixed payload area. On the current local Unix
reference platform, the total packet size is 10 bytes.

| Type byte | Name     | Payload format                              |
|-----------|----------|---------------------------------------------|
| `0x00`    | `push`   | `len` bytes from payload written into the PTY |
| `0x01`    | `attach` | `len != 0` means skip ring replay |
| `0x02`    | `detach` | no payload semantics |
| `0x03`    | `winch`  | payload carries `winsize` |
| `0x04`    | `redraw` | `len` carries redraw method; payload carries `winsize` |
| `0x05`    | `kill`   | `len` carries signal value |

Unknown type bytes should be treated as invalid packets and the connection
closed rather than guessed at. `len` is a single byte, not a 16-bit field.

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

The session may be considered started only after all three of these conditions
hold:

- the control socket is created, bound, listening, and connectable
- the PTY child-start path has succeeded, including exec-error handshake
  success with the daemon child
- a fresh client can connect to the socket

This rule applies to detached `start` semantics and to any internal readiness
handshake used between the CLI process and the master.

### Stale Socket Definition

A socket is **stale** when the socket file exists on the filesystem but no
master process is listening on it. This is detected by attempting to connect:
if `connect()` returns `ECONNREFUSED`, the socket is stale. A missing socket
file is not a stale socket — it is an absent session.

If the path exists but is not a socket, `connect()` resolution shall surface an
invalid-session / `ENOTSOCK` hard error rather than stale-session recovery.

No other `connect()` error implies stale recovery. Errors such as `ETIMEDOUT`
or `EPERM` are hard failures and remain ordinary command errors.

Stale sockets arise when the master process exits without cleaning up (e.g.
crash, power loss, SIGKILL). The log file and session directory remain valid
after a stale socket is detected and log replay must still be possible.

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

The watcher must not inject every ATM message into the session — it must filter
to messages intended for the current session or agent identity.

**Relevance criteria** (evaluated in order, first match wins):

1. **Explicit session address**: the ATM message `to` field matches the session
   name or a configured identity alias for this session.
2. **Ambient identity**: if no explicit address is present, the message is
   relevant if the `to` field matches the current OS user identity (same user
   that owns the session), and no other filter excludes it.
3. **Exclusion**: messages sent by the session itself (where `from` matches the
   session identity) must be suppressed to prevent feedback loops.

The session identity is derived from the `SCTERM_SESSION` environment variable
chain and the session name. The watcher receives the session name and socket
path as configuration at startup — it does not read `ATM_HOME` or walk the ATM
directory structure.

The relevance filter is local to `scterm-atm` and must be explicitly tested.
Unfiltered message injection is not acceptable.

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

The injected payload should use a deterministic, minimal envelope. For example:

```text
[ATM from <sender>]
<message text>
<CR>
```

The exact formatting can evolve, but these properties are required:

- sender identity is preserved;
- message text is preserved;
- unsafe control bytes are removed or escaped;
- the final carriage return is always present.

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

Architecture rules:

- `scterm-core` exposes `ScError` as a public error struct with contextual
  fields and backtrace support;
- `ScError` may use an internal private or non-exhaustive kind classification;
- public helper methods or accessors expose actionable conditions such as
  session-not-found, stale-socket, self-attach-loop, no-tty, invalid-session
  name, invalid-path, and log-cap-parse;
- ATM-specific failures are not part of `scterm-core` and are handled in
  `scterm-atm` or at the app boundary;
- `scterm-app` may use `anyhow` or one equivalent application error strategy,
  but library crates do not.

This keeps `core` future-proof and prevents ATM concerns from leaking into the
domain layer.

### RBP-2 (Blocking) — Coarse Typestate Lifecycle

Use typestate where it removes invalid public transitions without turning every
internal phase into type noise.

**Required coarse states**:

```text
Session (master side):
  Resolved -> Running
  Resolved -> Stale        (socket file exists and connect() returns ECONNREFUSED)

Attach client:
  LogReplaying -> Connecting -> RingReplaying -> Live -> Detached
```

Note the attach client ordering: log replay reads the on-disk log file
directly (no socket connection required) and must occur before the socket
connection is established. This matches `atch` behavior and allows history
replay even for stale sessions.

The `Stale` state is a terminal state for `Session<Resolved>` when the socket
is detected as stale. The caller must decide whether to remove the stale socket
and create a fresh `Session<Resolved>` (default open mode) or fail
(`attach` command).

`Starting`, `Exiting`, and `Exited` are valid internal operational phases, but
the coarse public typestate states remain `Resolved`, `Running`, and `Stale`.
`Running` specifically means the control socket is created/bound/listening, the
PTY child-start path succeeded, and a fresh client can connect.

Replay internals may stay private implementation detail if that keeps the API
clearer, but public lifecycle transitions should be consuming transitions rather
than ad-hoc mutable state checks.

Illustrative pattern:

```rust
pub struct Session<S> {
    inner: SessionInner,
    _state: PhantomData<S>,
}

impl Session<Resolved> {
    pub fn start(self, ...) -> Result<Session<Running>, ScError> { ... }
}

impl Session<Stale> {
    pub fn recover(self) -> Result<Session<Resolved>, ScError> { ... }
}
```

### RBP-3 (Important) — Sealed Platform Traits

All platform abstraction traits (`PtyBackend`, `SocketTransport`, and any
equivalent) must be sealed from day one using the standard sealed-trait pattern:

```rust
mod sealed { pub trait Sealed {} }

pub trait PtyBackend: sealed::Sealed { ... }
pub trait SocketTransport: sealed::Sealed { ... }
```

This is zero-cost and prevents downstream crates from implementing the traits,
preserving the ability to evolve the abstractions without breaking changes.

### RBP-4 (Important) — Domain Newtypes and Builder Shapes

Do not pass raw primitives across API boundaries. Define these newtypes in
`scterm-core`:

| Newtype | Wraps | Validated by constructor |
|---------|-------|--------------------------|
| `SessionName` | `String` | no `/`, non-empty, valid chars |
| `SessionPath` | `PathBuf` | absolute path, non-empty |
| `LogCap` | `u64` | accepts bare bytes and `k`/`K`/`m`/`M` suffixes, `0` = disabled |
| `RingSize` | `usize` | non-zero |

All constructors return `Result<T, ScError>` using the appropriate variant.
`LogCap::disabled()` is a named constructor for the zero case.

Configuration paths with four or more semantically distinct inputs should use a
builder or grouped config type instead of long positional constructors.

### RBP-5 (Blocking) — Documentation and API Hygiene

Public Rust APIs are contractual surface area.

Architecture rules:

- every public crate has crate docs;
- every public module has `//!` docs;
- public items have summary sentences and canonical doc sections where
  applicable;
- important public APIs include examples;
- re-exports that form part of the primary API use `#[doc(inline)]`.

This is not optional documentation polish. It is part of making the codebase
navigable and safe for both humans and agents.

### RBP-6 (Blocking) — Lints and Tooling Gates

The workspace should enable linting policy before feature work starts.

Architecture rules:

- workspace `Cargo.toml` defines `lints.rust` and `lints.clippy`;
- `cargo fmt`, `cargo clippy -D warnings`, and tests are required local and CI
  gates;
- `allow` and `expect` attributes require explicit reasons;
- dependency-health tools such as `cargo-audit`, `cargo-hack`, and `cargo-udeps`
  are planned as early follow-up gates.

### RBP-7 (Blocking) — Unsafe Containment

Unsafe code is a platform implementation detail, not a general-purpose escape
hatch.

Architecture rules:

- `scterm-core`, `scterm-app`, and `scterm-atm` contain no `unsafe` in Sprint 1;
- any `unsafe` required for Unix runtime integration is isolated to
  `scterm-unix`;
- each `unsafe` block documents its invariants and soundness argument;
- unsafe-bearing units are kept narrow enough for focused testing and Miri where
  practical.

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
