# scterm Requirements

## Purpose

This document defines the product requirements for `scterm`.

Related documents:

- `architecture.md`
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

Crate-level requirements are owned by the per-crate documents:

- `scterm-core/requirements.md`
- `scterm-unix/requirements.md`
- `scterm-app/requirements.md`
- `scterm-atm/requirements.md`

## Requirement ID Namespace

Product-level requirements use the prefix `REQ-TERM-*`.
Product-level architecture decisions use the prefix `ADR-TERM-*`.
Crate-level namespaces are defined in each crate's `requirements.md`.

The existing `REQ-RBP-*` IDs established in the Rust Implementation
Requirements section are stable and must not be renumbered.

`scterm` is a fresh Rust implementation of the `atch` session manager model,
with a tightly scoped first sprint:

1. Duplicate `atch` behavior on macOS and Linux.
2. Preserve the defining property of raw PTY passthrough with no terminal
   emulation.
3. Add ATM message delivery only after `atch` parity is established.

The `atch` source tree and its `tests/test.sh` integration script are the
behavioral reference for sprint 1.

## Product Statement

`scterm` shall be a transparent terminal session manager and interceptor that:

- lets a user create, detach from, and reattach to long-running terminal
  programs;
- preserves terminal compatibility by forwarding raw bytes rather than
  emulating a terminal;
- stores session output persistently on disk and replays it on reattach; and
- later supports inbound ATM messages as synthesized terminal input for
  agent-driven workflows.

## Core Principles

- No terminal emulation in the data path.
- Rust-native implementation, but behaviorally compatible with `atch`.
- Core terminal behavior must work without ATM.
- No `agent-team-mail-*` crate dependencies.
- No `ATM_HOME` references.
- No `use agent_team_mail::` or `use atm_*::` imports.
- ATM integration, when added, must remain an adapter at the process boundary.

## Non-Goals

The following are out of scope for the initial implementation:

- panes, windows, layouts, or terminal multiplexing features associated with
  `screen` or `tmux`;
- terminal escape-sequence parsing or translation;
- network-transparent remoting;
- Windows support in sprint 1;
- outbound ATM composition or a full ATM client UI;
- shell-specific prompt integration beyond compatibility with existing shells.

## Sprint Plan

### Sprint 1

Deliver behavioral parity with `atch` on macOS and Linux.

### Sprint 2

Add optional inbound ATM message integration that injects messages into the
session PTY as synthesized terminal input, terminated with carriage return to
nudge waiting agent terminals into action.

## Sprint 1 Functional Requirements

### Session Model

- A session shall be identified by a user-provided name or explicit filesystem
  path.
- A session name without `/` shall resolve under a per-user session directory.
- A session name containing `/` shall be treated as a direct path.
- The default session directory shall be `$HOME/.cache/<binary-name>`.
- If `$HOME` is unset or empty, the implementation shall fall back to the user
  database home directory.
- `$HOME=/` shall be treated as unusable for the default session directory and
  shall trigger the fallback path.
- If no usable home directory is available, the implementation shall fall back
  to `/tmp/.<binary-name>-<uid>`.
- Support for session paths that exceed the platform `sun_path` limit remains a
  product requirement, but the Unix-specific workaround is owned by
  `scterm-unix/requirements.md` (`REQ-TERM-UNIX-004`).
- Ownership note: validated session-name/path types are owned by
  `scterm-core/requirements.md` (`REQ-TERM-CORE-003`), while user-visible
  command/session expansion is owned by `scterm-app/requirements.md`.

### Commands

`scterm` shall support the following command behaviors with `atch`-compatible
semantics:

- `scterm [<session> [command...]]`
- `scterm attach <session>`
- `scterm new <session> [command...]`
- `scterm start <session> [command...]`
- `scterm run <session> [command...]`
- `scterm push <session>`
- `scterm kill [-f|--force] <session>`
- `scterm clear [<session>]`
- `scterm list`
- `scterm current`

The following aliases shall be supported:

- `attach` -> `a`
- `new` -> `n`
- `start` -> `s`
- `push` -> `p`
- `kill` -> `k`
- `list` -> `l`, `ls`

Legacy single-letter compatibility modes shall be supported for parity with
`atch`:

- `-a`, `-A`, `-c`, `-n`, `-N`, `-p`, `-k`, `-l`, `-i`

### Command Semantics

- Default open mode shall attempt attach first and create the session if the
  socket is missing or stale.
- `attach` shall fail if the session does not exist.
- `new` shall create a session and immediately attach.
- `start` shall create a session detached and return after startup succeeds.
  Startup success requires all three: the control socket is created, bound, and
  listening; the PTY child-start path succeeded; and a fresh client can
  connect to the socket. The `start` command must verify socket readiness by
  attempting to connect before returning exit code 0; if any condition fails
  within the startup window, `start` shall return a non-zero exit code.
- `run` shall create a session without daemonizing the master process.
- `push` shall copy stdin verbatim into the running session.
- `kill` shall send `SIGTERM` first, then escalate to `SIGKILL` after a grace
  period.
- `kill -f` and `kill --force` shall skip the grace period and send `SIGKILL`
  immediately.
- `clear` shall truncate the on-disk session log.
- `current` shall print the human-readable session ancestry chain and exit
  successfully only when inside a session.
- `list` shall distinguish running sessions, running sessions with at least one
  attached client, and stale sessions.
- Attached state for `list` shall be represented by master-owned session
  metadata equivalent in behavior to `atch`'s socket execute-bit marker.
- The master shall set and clear attached-state metadata as clients connect and
  disconnect.

### Option Handling

The following options shall be supported with `atch`-compatible meaning:

- `-e <char>`
- `-E`
- `-r <none|ctrl_l|winch>`
- `-R <none|move>`
- `-z`
- `-q`
- `-t`
- `-C <size>`

Option handling requirements:

- Global options shall work before the subcommand.
- Options shall also work after the subcommand and before or after the session
  name when `atch` supports that placement.
- Stacked short options such as `-qEzt` shall be accepted when valid.
- `--` shall stop option parsing and pass remaining arguments to the child
  command unchanged.
- `-C` shall accept bare bytes and `k`/`K` or `m`/`M` suffixes.
- `-C 0` shall disable on-disk logging for newly created sessions.

### Terminal Behavior

- `scterm` shall not emulate a terminal.
- The child process shall observe the real terminal type unchanged.
- Mouse reporting, colors, graphics, alternate-screen behavior, and OSC
  sequences shall pass through unmodified.
- Attach clients shall place the user terminal in raw mode while attached and
  restore the original settings on exit, detach, or suspend.
- The default detach character shall be `Ctrl-\`.
- The detach character shall be configurable and may be disabled.
- Suspend-key handling shall match `atch` behavior, including detach before
  suspend and reattach plus redraw after resume.
- `SIGWINCH` handling shall forward terminal size changes to the session PTY.

### PTY and Client Behavior

- The master process shall own the PTY and child process lifecycle.
- Multiple clients shall be able to attach concurrently to the same session.
- Attached clients shall receive the same raw PTY output stream.
- Client input shall be forwarded into the PTY as raw bytes.
- The master shall continue running after clients detach, until the child exits
  or the session is explicitly killed.

### Session Lifecycle

- Master startup shall create, bind, and listen on the control socket before
  the PTY fork/exec path begins.
- The session shall not be considered `Running` until all three of these hold:
  the control socket exists, is bound, and is listening; the PTY child-start
  path succeeded; and a fresh client can connect to the socket.
- The master shall not broadcast PTY output before the first client attaches.
  Output produced before first attach shall still be captured to the
  persistent log and in-memory ring buffer.
- On first attach, the client shall receive history via log replay and ring
  replay before transitioning to live streaming.
- Ownership note: the coarse lifecycle states and stale-session domain
  conditions are owned by `scterm-core/requirements.md`
  (`REQ-TERM-CORE-005`), runtime socket/PTY primitives are owned by
  `scterm-unix/requirements.md` (`REQ-TERM-UNIX-004`,
  `REQ-TERM-UNIX-005`), and startup/attach choreography is owned by
  `scterm-app/requirements.md` (`REQ-TERM-APP-005`).

### Structured Logging

- Structured logging is a product requirement, but the logging implementation
  and lifecycle are owned by `scterm-app/requirements.md`
  (`REQ-TERM-APP-003`).
- The decision to standardize `scterm-app` on an
  `sc-observability`-backed `AppLogger` is recorded in
  `scterm-app/architecture.md` (`ADR-TERM-APP-006`).
- The product-level contract is:
  - local structured logs remain available for debugging and CI diagnostics
  - lower crates do not own logger lifecycle or sink configuration
  - `scterm-app` shall use `sc-observability` as the mandated backend
  - lower crates remain observability-backend agnostic

### Multi-Client Detach and Kill Semantics

- When one client detaches, all other attached clients shall remain connected
  and continue receiving PTY output uninterrupted.
- When `kill` is issued, the master shall send `SIGTERM` to the child process
  group. All currently attached clients shall be disconnected immediately with
  their local terminals restored before the master exits.
- When `kill -f` / `kill --force` is issued, the master shall send `SIGKILL`
  to the child process group. All clients shall be disconnected immediately.
- When the child process exits for any reason (natural exit, signal, kill),
  the master shall disconnect all attached clients, write the end marker to the
  log, clean up the socket file, and exit.
- A client that is mid-attach (replaying log or ring) when the session is
  killed shall have its connection closed cleanly and its terminal restored.

### Session History

- Session output shall be recorded in an in-memory ring buffer.
- Session output shall also be recorded in an on-disk log unless logging is
  disabled.
- On attach, the client shall replay the on-disk log first.
- The master shall replay the in-memory ring buffer after attach unless the
  client already replayed equivalent history from disk.
- The on-disk log shall persist across detach, crash, and reboot.
- When a session exits cleanly, an end marker shall be appended to the log.
- When a stale socket is encountered, log replay shall still be possible.
- The default ring buffer size shall be 128 KiB.
- The default log cap shall be 1 MiB.
- Ownership note: portable history primitives and replay-order rules are owned
  by `scterm-core/requirements.md` (`REQ-TERM-CORE-006`), while log file I/O
  and replay choreography are owned by `scterm-app/requirements.md`
  (`REQ-TERM-APP-005`).

### Environment and Nesting

The ancestry environment contract is normative Sprint 1 behavior and shall
match `atch`.

- The ancestry chain shall impose no fixed nesting-depth cap in Sprint 1.
- `current` shall render the ancestry chain in human-readable order.
- `clear` with no explicit session argument shall target the innermost session
  from that chain.
- Self-attach prevention shall remain a domain-layer predicate, not a CLI-local
  guard.
- Ownership note: the exact ancestry variable derivation, parsing/rendering
  rules, and self-attach algorithm are owned by
  `scterm-core/requirements.md` (`REQ-TERM-CORE-004`); the CLI/app layer only
  maps those typed outcomes into user-visible messages and exit codes through
  `scterm-app/requirements.md` (`REQ-TERM-APP-007`).

### Security and Isolation

- Session sockets and log files shall be created with owner-only permissions.
- Session discovery and access control shall rely on filesystem permissions.
- No authentication protocol shall be added in sprint 1.
- Tests shall isolate session state in temporary directories.

### Stale Socket Definition

A socket is **stale** when the socket file exists on the filesystem but
`connect()` on it returns `ECONNREFUSED`. A missing socket file is not a
stale socket; it is an absent session. Log replay is still valid and possible
after a stale socket is detected.

If the path exists but is not a socket, the implementation shall surface
`ENOTSOCK` / invalid-session behavior and shall not attempt stale recovery.

No other `connect()` error implies stale recovery. Errors such as `ETIMEDOUT`
or `EPERM` are hard failures and shall not be treated as stale.

### Error Handling and UX

- Error messages and exit codes shall be broadly compatible with `atch`.
- Invocations requiring a TTY shall fail clearly when no TTY is present.
- Empty-session `list` shall print `(no sessions)` unless quiet mode suppresses
  the meta-message.
- Stale sessions shall be detectable and shown distinctly by `list`.
- Bad commands, missing sessions, invalid options, and invalid argument counts
  shall all produce deterministic failures.

### Exit Codes

The following exit codes are required:

| Code | Meaning |
|------|---------|
| `0`  | Success |
| `1`  | General / unclassified error |
| `2`  | Usage error (bad command, invalid option, missing argument) |
| `3`  | Session not found |
| `4`  | Session is stale (socket exists but master is not running) |
| `5`  | Self-attach loop detected |
| `6`  | No TTY available (operation requires a terminal) |
| `7`  | Session already exists (when `new` is used and session is running) |
| `8`  | `start` startup timeout (socket not connectable within timeout) |

Exit codes must be stable across releases. New codes may be added; existing
codes must not be reassigned.

## Sprint 1 Acceptance Criteria

- The Rust implementation shall pass a ported compatibility suite derived from
  `atch/tests/test.sh`.
- Compatibility coverage shall include command parsing, aliases, legacy modes,
  session path rules, log behavior, kill semantics, `current`, `clear`, quiet
  mode, stale session handling, and non-TTY behavior.
- The implementation shall build and test successfully on macOS and Linux.
- The implementation shall satisfy the repository cross-platform rules in
  `cross-platform-guidelines.md`.

## Sprint 2 ATM Integration Requirements

### Scope

Sprint 2 adds inbound ATM delivery into active terminal sessions without
changing sprint 1 session semantics.

### ATM Message Relevance Filter

The ATM watcher shall only inject messages that are relevant to the current
session. Injecting all inbound ATM messages is not acceptable.

Relevance is determined as follows:

- A message is relevant if its `to` field matches the session name or a
  configured identity alias for this session.
- If no explicit address match exists, a message is relevant if its `to` field
  matches the current OS username that owns the session, and no exclusion
  applies.
- A message whose `from` field matches the session identity shall be excluded
  to prevent feedback loops.

The session identity (name and socket path) is passed to the watcher at
startup. The watcher shall not read `ATM_HOME` or walk ATM internal directories
to resolve identity.

This filter shall be independently testable without a live ATM installation.

### Hard Constraints

- ATM integration shall remain optional.
- Core session behavior shall remain usable when ATM is unavailable.
- The implementation shall use the external `atm` CLI, not ATM Rust crates.
- The preferred receive path shall be a blocking CLI read such as
  `atm read --timeout ...` or an equivalent ATM CLI subscription mechanism.
- The implementation shall not read `ATM_HOME`.
- Any ATM-specific state written by `scterm` shall use `SCTERM_*` names or
  session-local files.

### Inbound Message Delivery

- `scterm` shall support a mode that watches for inbound ATM messages relevant
  to the current agent/session.
- On inbound ATM delivery, `scterm` shall synthesize terminal input for the
  child PTY containing the inbound message payload.
- The synthesized input shall terminate with carriage return so blocked
  agent-oriented terminals are nudged into processing the new input.
- Delivery shall be serialized with ordinary user input and `push` traffic so
  PTY writes remain ordered.
- Delivery shall occur once per message per session.
- If multiple clients are attached, the injected message shall still be written
  only once to the PTY.

### Message Format

- The injected payload shall include sender identity and message text.
- Control bytes other than newline, carriage return, and tab shall be removed
  or escaped before injection.
- The formatting shall be deterministic so agents can reliably respond to it.

### Failure Handling

- If the `atm` CLI is unavailable or returns an error, the session shall remain
  usable.
- ATM watcher failure shall not terminate the master process or child PTY.
- Duplicate message injection after reconnect or restart shall be prevented by
  explicit local tracking.

## Rust Implementation Requirements

These requirements apply to Sprint 1 implementation and are non-negotiable.
They were established by pre-sprint RBP design review.

### REQ-RBP-001 — Library and Application Error Policy (Blocking)

The implementation shall follow a library/application split for error handling.

- `scterm-core` and `scterm-unix` are library crates and shall expose typed
  library errors rather than `anyhow` or equivalent application-level errors.
- `scterm-app` and the final binary may use `anyhow` as the application error
  boundary, but only one application-level error strategy shall be used across
  those crates.
- ATM-specific failures such as missing CLI or watcher crashes shall not be
  represented in `scterm-core`; they belong in `scterm-atm` and the app layer.
- The CLI layer shall map application-facing failures to deterministic exit
  codes.
- Ownership note: concrete `ScError` shape and core-library error rules live in
  `scterm-core/requirements.md` (`REQ-TERM-CORE-001`,
  `REQ-TERM-CORE-002`), Unix-library error rules live in
  `scterm-unix/requirements.md`, and the single app-boundary strategy plus exit
  mapping live in `scterm-app/requirements.md`
  (`REQ-TERM-APP-001`, `REQ-TERM-APP-007`).

### REQ-RBP-002 — Public Documentation Standard (Blocking)

All public Rust surface area shall be documented from the first code-bearing
commit.

- Every public crate shall have crate-level documentation.
- Every public module shall have `//!` module documentation.
- Every public type and function shall have a short summary sentence.
- Public items shall include canonical Rust doc sections when applicable:
  `# Examples`, `# Errors`, `# Panics`, `# Safety`, and `# Abort`.
- Key user-facing and extension-facing APIs shall include directly usable
  examples.
- Re-exported internal items intended to appear as primary API shall use
  `#[doc(inline)]`.
- Ownership note: crate-local API documentation obligations are further
  tracked in the per-crate requirement docs and in `public-api-checklist.md`.

### REQ-RBP-003 — Typestate at Coarse Lifecycle Boundaries (Blocking)

The implementation shall use typestate where it materially prevents invalid
session lifecycle transitions.

- Replay phases may use typestate or private internal state as long as invalid
  public transitions remain unrepresentable or tightly constrained.
- State transitions exposed across public API boundaries shall consume the old
  state and return the new state.
- Ownership note: the concrete state set and transition obligations are owned
  by `scterm-core/requirements.md` (`REQ-TERM-CORE-005`).

### REQ-RBP-004 — Sealed Platform Traits (Important)

Platform abstraction traits (`PtyBackend`, `SocketTransport`, and equivalents)
shall be sealed using the `mod sealed` pattern from day one. External crates
must not be able to implement these traits.

Ownership note: the concrete trait boundary is owned by
`scterm-unix/requirements.md` (`REQ-TERM-UNIX-001`).

### REQ-RBP-005 — Domain Newtypes and Builders (Important)

The implementation shall define `SessionName`, `SessionPath`, `LogCap`, and
`RingSize` as newtypes in `scterm-core`. Raw strings, `PathBuf`, and numeric
primitives shall not be passed across public API boundaries in their place.

- Configuration or constructor paths requiring four or more semantically
  distinct parameters shall use a builder or grouped config type rather than a
  long positional parameter list.
- Ownership note: the concrete newtype and builder obligations are owned by
  `scterm-core/requirements.md` (`REQ-TERM-CORE-003`).

### REQ-RBP-006 — Lints and Tooling from Day One (Blocking)

The workspace shall enable Rust and Clippy lint configuration from the first
commit that introduces Rust code.

- Workspace `Cargo.toml` files shall define explicit `[lints.rust]` and
  `[lints.clippy]` sections.
- `cargo fmt --all` shall be required.
- `cargo clippy --all-targets --all-features -- -D warnings` shall be required.
- Any `#[allow(...)]` or `#[expect(...)]` must include a reason.
- Additional tools such as `cargo-audit`, `cargo-hack`, and `cargo-udeps`
  should be added as early as practical, but linting, formatting, and tests are
  the immediate blocking gates.
- The ATM boundary check (grep scan for `agent.team.mail`, `agent_team_mail`,
  `atm_`, `use atm::`, `ATM_HOME` in `*.rs` and `Cargo.toml`) is a CI gate
  from day one. See `../.github/workflows/ci.yml`.
- CI runs on ubuntu-latest and macos-latest to enforce cross-platform
  compliance from the first code-bearing commit.

Ownership note: crate-local lint and API-hygiene compliance is further tracked
by `public-api-checklist.md` and the per-crate requirement docs.

### REQ-RBP-007 — Unsafe Containment Policy (Blocking)

Unsafe Rust shall be treated as an exception path, not a convenience tool.

- `scterm-core`, `scterm-app`, and `scterm-atm` shall contain no `unsafe`
  blocks in Sprint 1.
- Ownership note: the Unix unsafe boundary is owned by
  `scterm-unix/requirements.md` (`REQ-TERM-UNIX-002`), while the explicit
  no-unsafe requirements for the other crates live in
  `scterm-core/requirements.md` (`REQ-TERM-CORE-007`),
  `scterm-app/requirements.md` (`REQ-TERM-APP-008`), and
  `scterm-atm/requirements.md` (`REQ-TERM-ATM-007`).

## Future Considerations

These items are intentionally deferred:

- Windows transport support
- richer ATM workflows such as outbound replies
- policy controls for message filtering, allowlists, or routing
- session sharing across machines
