# scterm-core Architecture

## Purpose

This document defines the crate-local architecture of `scterm-core`.

Product architecture is owned by `../architecture.md`. This document covers
the module structure, internal design decisions, and crate-level ADRs for
`scterm-core`.

## Module Responsibilities

The following is the expected module structure. Exact layout is authoritative
in `scterm-core/src/`.

- `error` — `ScError` struct, kind classification, contextual accessors
- `session` — `SessionName`, `SessionPath`, validated constructors, and
  portable path rules after CLI/session expansion
- `ring` — in-memory ring buffer
- `packet` — client-to-master packet definitions and validator logic
- `state` — session master and attach client state types and consuming
  transitions
- `ancestry` — ancestry environment variable naming, chain parsing/rendering,
  and self-attach predicate
- `config` — `LogCap`, `RingSize`

## Control Packet Contract

The client-to-master control packet model is owned by `scterm-core`.

All client-to-master packets share a common fixed layout compatible with
`atch`:

```text
Offset  Size   Field
------  -----  -----
0       1      packet type (u8, see table below)
1       1      length / selector byte (u8)
2       8      fixed payload area (`sizeof(struct winsize)` on the reference platform)
```

This is a 2-byte header plus fixed payload area. On the current local Unix
reference platform, the total packet size is 10 bytes.

| Type byte | Name     | Payload format |
|-----------|----------|----------------|
| `0x00`    | `push`   | `len` bytes from payload written into the PTY |
| `0x01`    | `attach` | `len != 0` means skip ring replay |
| `0x02`    | `detach` | no payload semantics |
| `0x03`    | `winch`  | payload carries `winsize` |
| `0x04`    | `redraw` | `len` carries redraw method; payload carries `winsize` |
| `0x05`    | `kill`   | `len` carries signal value |

Unknown type bytes are invalid packets. `len` is a single byte, not a 16-bit
field, and its semantics depend on packet type.

## Dependency Rule

This crate has no dependencies outside the Rust standard library and
well-audited, platform-agnostic utilities approved in `../dependency-policy.md`.

See `../crate-boundaries.md` for the enforced dependency direction.

## ADR-TERM-CORE-001 — ScError as Struct, Not Enum

`ScError` is a struct with contextual fields and a private kind discriminant.
External callers use accessor methods and helper predicates rather than
exhaustive matching on error variants.

This prevents callers from coupling to internal error taxonomy and allows the
error surface to evolve without breaking API.

## ADR-TERM-CORE-002 — Typestate Consumes Old State

State transitions exposed across the public API boundary consume the old state
value and return the new state. Invalid transitions are unrepresentable at the
type level.

See REQ-RBP-003 in `../requirements.md`.

## ADR-TERM-CORE-003 — Domain Rules Stay Portable

The crate owns the portable rule set that higher layers consume, but not the
OS calls that discover runtime facts.

Examples:

- `scterm-core` owns stale-session classification as a domain condition, but
  not Unix socket I/O
- `scterm-core` owns ancestry parsing and self-attach detection, but not CLI
  exit-code rendering
- `scterm-core` owns log-cap and ring-size semantics, but not log file I/O

See `requirements.md` REQ-TERM-CORE-004 through REQ-TERM-CORE-006.
