# scterm Public API Checklist

## Purpose

Track the intended public API so implementation does not invent or expand the
surface opportunistically.

## Usage

- `[ ]` pending
- `[~]` designed, not implemented
- `[x]` finalized and implemented

## `scterm-core`

- [~] `SessionName`
- [~] `SessionPath`
- [~] `LogCap`
- [~] `RingSize`
- [~] `ScError`
- [~] packet types and redraw/clear enums
- [~] ancestry helpers
- [~] ring buffer type
- [~] coarse state marker types

## `scterm-unix`

Public API should stay narrow.

- [~] sealed PTY backend trait
- [~] sealed socket transport trait
- [~] raw terminal guard
- [~] Unix runtime error type

Internal-only:

- [~] daemonization helpers
- [~] signal wiring details

## `scterm-app`

- [~] session/master orchestration entrypoints
- [~] attach-client orchestration entrypoints
- [~] command dispatch surface

Internal-only:

- [~] logger wiring via `sc-observability`
- [~] PTY input serialization queue

## `scterm-atm`

- [~] ATM watcher service
- [~] typed inbound message event
- [~] adapter error type

Internal-only:

- [~] dedupe persistence details
- [~] raw CLI parsing helpers
