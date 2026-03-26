# scterm Protocol

## Purpose

This document defines the Sprint 1 session wire behavior.

## Compatibility Rule

The protocol is intentionally modeled on `atch`.

If implementation wants to change the protocol, it must first change this
document and the compatibility decision that depends on it.

## Directionality

The protocol is asymmetric.

### Client to Master

Client-to-master uses a fixed packet.

Fields:

- `type: u8`
- `len: u8`
- `payload: [u8; sizeof(winsize)]`

On the current local Unix reference platform:

- `sizeof(winsize) = 8`
- packet size = 10 bytes

This exact byte size is platform-sensitive and should be treated as an ABI
compatibility target for the supported Unix platforms rather than guessed from
research notes.

### Master to Client

Master-to-client uses a raw byte stream with no message framing.

That is a core compatibility decision. The master is not a terminal protocol
interpreter.

## Packet Types

| Name | Value | Meaning |
|---|---:|---|
| `Push` | 0 | PTY input bytes from client |
| `Attach` | 1 | attach request |
| `Detach` | 2 | detach notification |
| `Winch` | 3 | terminal resize |
| `Redraw` | 4 | redraw request |
| `Kill` | 5 | signal request |

## Packet Semantics

The `len` field serves different semantic roles depending on packet type: a
byte count for `Push`, a boolean flag for `Attach`, a method enum for
`Redraw`, and a signal value for `Kill`. Treat each packet type's semantics
independently.

### `Push`

- `len` is the number of valid bytes in `payload`
- bytes are written to the PTY by the master

### `Attach`

- `len = 0` means normal attach with ring replay allowed
- `len != 0` means the client already replayed log history and ring replay
  should be skipped

### `Detach`

- marks the client detached without stopping the master

### `Winch`

- `payload` carries the terminal `winsize`

### `Redraw`

- `len` carries redraw method
- `payload` carries current `winsize`

### `Kill`

- `len` carries the requested signal value

## Redraw Methods

| Name | Value |
|---|---:|
| `Unspecified` | 0 |
| `None` | 1 |
| `CtrlL` | 2 |
| `Winch` | 3 |

## Clear Methods

These are CLI/config semantics, not packet semantics.

| Name | Value |
|---|---:|
| `Unspecified` | 0 |
| `None` | 1 |
| `Move` | 2 |
