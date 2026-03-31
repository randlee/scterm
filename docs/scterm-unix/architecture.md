# scterm-unix Architecture

## Purpose

This document defines the crate-local architecture of `scterm-unix`.

Product architecture is owned by `../architecture.md`. This document covers
the module structure, internal design decisions, and crate-level ADRs for
`scterm-unix`.

## Module Responsibilities

The following is the expected module structure. Exact layout is authoritative
in `crates/scterm-unix/src/`.

- `pty` — PTY creation, child spawn, read/write/resize, ownership
- `socket` — Unix domain socket lifecycle (bind, connect, listen, accept,
  cleanup, `chdir`-based long-path workaround)
- `rawmode` — termios raw-mode guard, RAII restoration
- `signal` — `SIGWINCH` propagation, `SIGTERM`/`SIGKILL` handling, `SIGCHLD`
  reaping
- `daemon` — daemonization mechanics (double-fork, setsid, fd redirection)

## Dependency Direction

`scterm-unix` depends on `scterm-core` for domain types.
It does not depend on `scterm-app` or `scterm-atm`.

See `../crate-boundaries.md` for the enforced dependency direction.

## ADR-TERM-UNIX-001 — No Product Semantics in Platform Code

Platform code implements narrow interfaces required by `scterm-core` and
`scterm-app`. It does not define or enforce product-level session semantics.
If a rule matters outside the platform layer, it belongs in `scterm-core`.

## ADR-TERM-UNIX-002 — Unsafe Bounded to PTY Module

If `unsafe` blocks are needed, they are confined to the `pty` module.
No other module in this crate shall use `unsafe`.

## ADR-TERM-UNIX-003 — Platform Facts In, Product Policy Out

This crate may discover Unix runtime facts and execute Unix-only mechanisms,
but it must not interpret them into product decisions.

Examples:

- it may report socket-connect outcomes, but it does not decide stale-session
  recovery policy
- it may expose raw-mode and resize primitives, but it does not own attach or
  detach semantics
- it may perform the long-path `chdir` workaround, but it does not decide how
  user-visible session names resolve

See `requirements.md` REQ-TERM-UNIX-004 and REQ-TERM-UNIX-005.
