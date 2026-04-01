# scterm Public API Checklist

## Purpose

Track the intended public API so implementation does not invent or expand the
surface opportunistically.

## Usage

- `[ ]` pending
- `[~]` designed, not implemented
- `[x]` finalized and implemented

## `scterm-core`

- [x] `SessionName`
- [x] `SessionPath`
- [x] `LogCap`
- [x] `RingSize`
- [x] `ScError`
- [x] packet types and redraw/clear enums
- [x] ancestry helpers
- [x] ring buffer type
- [x] coarse state marker types

## `scterm-unix`

Public API should stay narrow.

- [x] sealed PTY backend trait
- [x] sealed socket transport trait
- [x] raw terminal guard
- [x] Unix runtime error type

Internal-only:

- [x] daemonization helpers
- [x] signal wiring details

## `scterm-app`

- [x] session/master orchestration entrypoints
- [x] attach-client orchestration entrypoints
- [x] command dispatch surface

Internal-only:

- [x] logger wiring via `AppLogger`
- [x] PTY input serialization queue

## `scterm-atm`

- [x] ATM watcher service
- [x] typed inbound message event
- [x] adapter error type

Internal-only:

- [x] dedupe persistence details
- [x] raw CLI parsing helpers
