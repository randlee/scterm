# scterm Error Model

## Purpose

This document prevents error-handling drift between crates.

## Principles

- library crates expose typed library errors
- the app layer owns the final application error boundary
- user-facing exit behavior is decided at the CLI/app boundary
- ATM-specific failures do not leak into `scterm-core`

## `scterm-core`

Public error surface:

- `ScError`

Properties:

- typed library error
- carries contextual fields
- supports actionable user-facing rendering upstream
- used by public constructors and domain operations

Typical conditions:

- invalid session name
- invalid path
- bad log-cap parse
- stale socket detected (socket file exists, `connect()` returns `ECONNREFUSED`)
- ancestry derivation and self-attach predicate failures
- protocol validation issues

`scterm-core` owns the ancestry derivation rules and the self-attach prevention
predicate itself, not just the shared error type used to report those failures.

## `scterm-unix`

Public error surface:

- `UnixError` or an equivalently named typed runtime error

Properties:

- typed library error
- contains Unix/runtime context
- does not own CLI exit-code decisions

Typical conditions:

- socket bind/connect failures
- PTY spawn failures
- raw-mode failures
- signal or resize errors

## `scterm-atm`

Public error surface:

- `AtmError` or an equivalently named typed adapter error

Typical conditions:

- `atm` binary unavailable
- CLI parse failure
- duplicate-state persistence failure

These errors stay in the adapter and app layers. They are not part of
`scterm-core`.

## `scterm-app`

Application boundary:

- may use `anyhow`
- converts typed library errors into command outcomes
- maps command outcomes into deterministic exit codes and stderr messages

## Exit-Code Ownership

Exit-code mapping belongs only to:

- `scterm-app`
- the final binary

It does not belong to:

- `scterm-core`
- `scterm-unix`
- `scterm-atm`
