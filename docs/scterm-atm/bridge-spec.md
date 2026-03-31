# scterm-atm Bridge Spec

## Purpose

This document defines the ATM extension as an adapter on top of the Sprint 1
session core.

Satisfies: product requirements in `../requirements.md` — Sprint 2 ATM
Integration Requirements section.

## Scope

The ATM bridge:

- watches for inbound ATM messages using the external `atm` CLI
- normalizes those messages into typed events
- injects them into the session PTY input stream through the master

The ATM bridge does not:

- own the PTY
- own session lifecycle
- require ATM for normal terminal usage

## Receive Path

Preferred receive path:

- blocking `atm read --timeout ...`
- or a semantically equivalent ATM CLI subscription mode

Busy polling is not acceptable.

## Injection Contract

Each inbound ATM message becomes one synthesized PTY input write sequence:

```text
[ATM from <sender>]
<message text>
\r
```

Required properties:

- sender identity is preserved
- message text is preserved after sanitization
- final carriage return is always present
- the write is serialized with all other PTY input sources

## Sanitization Rules

- remove or escape control characters other than `\n`, `\r`, and `\t`
- preserve printable text faithfully
- do not execute message content as shell syntax
- use deterministic formatting

## De-Duplication

The bridge shall maintain local per-session delivery tracking.

Rules:

- each inbound message is injected at most once per session
- reconnecting clients do not cause reinjection
- watcher restart does not blindly replay already delivered messages

## Failure Policy

- ATM unavailable: session remains usable
- watcher error: master and PTY remain alive
- parse failure: drop the single message, not the session
- injection failure: report locally, do not corrupt the PTY stream

## Logging Policy

ATM bridge activity should be logged through the app-owned `sc-observability`
logger, not through direct stdout/stderr noise in the session.
