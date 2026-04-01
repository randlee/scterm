# scterm-unix Requirements

## Purpose

This document defines what `scterm-unix` owns and how it satisfies referenced
product requirements.

Product requirements are owned by `../requirements.md`. This document must not
restate the product contract — it references it.

## Requirement ID Namespace

Crate-level requirements use the prefix `REQ-TERM-UNIX-*`.
Crate-level architecture decisions use the prefix `ADR-TERM-UNIX-*`.

## What This Crate Owns

`scterm-unix` owns all platform-specific Unix runtime behavior:

- Unix socket bind, connect, listen, accept, and cleanup
- PTY creation, child-process spawn, read, write, and resize
- raw-mode terminal guard (termios acquisition and restore)
- signal handling (`SIGWINCH`, `SIGTERM`, `SIGKILL`, `SIGCHLD`)
- daemonization mechanics

## What This Crate Must Not Own

- Product semantics or session lifecycle policy
- Application-level structured logging configuration
- ATM-specific parsing or integration
- CLI command parsing or user-facing message rendering

## Product Requirements Satisfied

The following product requirements from `../requirements.md` are implemented
by this crate:

- Session Model section — long `sun_path` workaround and Unix filesystem
  operations needed to support deep session paths
- PTY and Client Behavior section — PTY ownership, raw-mode, resize propagation
- Session Lifecycle section — socket lifecycle primitives and daemon/runtime
  hand-off primitives
- Terminal Behavior section — raw-mode entry and exit, suspend handling
- REQ-RBP-004 — sealed platform traits: `PtyBackend`, `SocketTransport`
- REQ-RBP-007 — unsafe containment: any `unsafe` blocks isolated here with
  explicit safety invariants

## REQ-TERM-UNIX-001 — Sealed Platform Traits

All platform abstraction traits in this crate (`PtyBackend`, `SocketTransport`,
and equivalents) shall be sealed using the `mod sealed` pattern.

Satisfies: REQ-RBP-004 in `../requirements.md`.

## REQ-TERM-UNIX-002 — Unsafe Isolation and Documentation

If `unsafe` is required in this crate, it shall be isolated to the smallest
possible module and documented with explicit safety invariants. Every `unsafe`
block shall carry a comment explaining why it is sound.

Satisfies: REQ-RBP-007 in `../requirements.md`.

## REQ-TERM-UNIX-003 — Raw-Mode Restoration on All Exit Paths

The raw-mode terminal guard shall restore original terminal settings on normal
exit, detach, suspend resume, and panic unwind.

Satisfies: Terminal Behavior section in `../requirements.md`.

## REQ-TERM-UNIX-004 — Unix Socket and Long-Path Support

`scterm-unix` shall own the Unix-specific socket mechanics needed by the
product contract:

- bind, connect, listen, accept, and cleanup for local control sockets
- parent-directory `chdir` plus basename-only bind/connect fallback when a
  validated session path exceeds the platform `sun_path` limit
- owner-only socket-file permissions and cleanup helpers

Satisfies: Session Model section, Session Lifecycle section, and Security and
Isolation section in `../requirements.md`.

## REQ-TERM-UNIX-005 — PTY and Signal Runtime Primitives

`scterm-unix` shall provide the Unix runtime primitives consumed by the app
layer:

- PTY open/spawn/read/write/resize
- process-group management and signal forwarding hooks
- daemonization and child-start handshake support used by detached startup

This crate does not decide product policy such as when a session is
considered running or how kill UX is rendered.

Satisfies: Terminal Behavior section, PTY and Client Behavior section, and
Session Lifecycle section in `../requirements.md`.
