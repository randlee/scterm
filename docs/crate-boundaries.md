# scterm Crate Boundaries

## Purpose

This document is the single-source boundary matrix for the `scterm` workspace.

If implementation and this file disagree, this file wins until explicitly
amended.

## Workspace Shape

Sprint 1 targets:

- `scterm-core`
- `scterm-unix`
- `scterm-app`

Sprint 2 adds:

- `scterm-atm`

The binary may live inside `scterm-app` or as a thin top-level package, but it
must not become a fourth behavior crate in Sprint 1.

## Ownership Matrix

### `scterm-core`

Owns:

- packet definitions
- session path rules
- session ancestry rules
- validated newtypes
- ring buffer implementation
- domain errors
- domain state-machine types
- ATM-independent message envelope types

May depend on:

- `std`
- small utility crates such as `thiserror`

Must not depend on:

- Tokio
- Unix socket APIs
- PTY APIs
- `sc-observability`
- `sc-observability-types`
- `atm`

Must not know about:

- daemonization
- terminal raw mode
- concrete socket paths beyond validated path rules
- sink configuration
- CLI rendering text

### `scterm-unix`

Owns:

- Unix socket transport
- PTY integration
- raw terminal mode
- signal handling
- process groups
- daemonization
- Unix-specific filesystem operations

May depend on:

- `scterm-core`
- Unix/runtime crates required for PTY and socket behavior

Must not depend on:

- `scterm-atm`
- CLI parser crates
- `sc-observability`
- `sc-observability-types`

Must not know about:

- ATM mailbox semantics
- command alias rules
- user-facing help text
- sink or logger configuration policy

### `scterm-app`

Owns:

- command orchestration
- master/client orchestration
- `atch` compatibility behavior
- structured logging integration
- PTY write ordering policy
- the only serialized write path into the PTY file descriptor
- application-level error boundary

May depend on:

- `scterm-core`
- `scterm-unix`
- `sc-observability`
- `sc-observability-types`
- one application error crate such as `anyhow`
- `serde_json` (via AppLogger)

Must not depend on:

- ATM Rust crates

Must not know about:

- later observability concerns beyond local structured logging
- ATM transport internals before Sprint 2

### `scterm-atm`

Owns:

- blocking reads from the external `atm` CLI
- inbound message normalization
- message de-duplication state
- typed ATM-to-app event translation

May depend on:

- `scterm-core`
- application support crates needed for the adapter

Must not depend on:

- `scterm-unix`
- PTY internals
- any external observability crate

Must not know about:

- socket protocol internals
- terminal raw-mode mechanics
- PTY ownership

## Logging Boundary

Structured logging in `scterm-app` uses `sc-observability` as the mandated
backend and `sc-observability-types` for the shared log type contracts. Lower
crates (`scterm-core`, `scterm-unix`, `scterm-atm`) remain backend-agnostic
and do not initialize or shut down the logging subsystem.

Boundary rules:

- only `scterm-app` and the final binary own and configure the `AppLogger`
- `scterm-core`, `scterm-unix`, and `scterm-atm` do not configure or own
  logger lifecycle
- lower crates prefer rich typed errors and return values over ad-hoc logging

## Boundary Review Questions

Every non-trivial change should answer these:

- Which crate owns this behavior?
- Why does it not belong one layer lower?
- Does this change introduce a new dependency edge?
- Does it pull ATM knowledge into a non-ATM crate?
- Does it pull runtime knowledge into `scterm-core`?
- Does it pull logging implementation knowledge into `scterm-core` or
  `scterm-unix`?
