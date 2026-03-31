# scterm-atm Requirements

## Purpose

This document defines what `scterm-atm` owns and how it satisfies referenced
product requirements.

Product requirements are owned by `../requirements.md`. This document must not
restate the product contract — it references it.

## Requirement ID Namespace

Crate-level requirements use the prefix `REQ-TERM-ATM-*`.
Crate-level architecture decisions use the prefix `ADR-TERM-ATM-*`.

## What This Crate Owns

`scterm-atm` owns the ATM bridge adapter:

- blocking reads from the external `atm` CLI
- inbound message relevance filtering
- normalization of inbound message text into typed events
- de-duplication state for delivered messages
- conversion from inbound ATM events to sanitized injection requests

See `bridge-spec.md` for the full injection contract, sanitization rules,
and failure policy.

## What This Crate Must Not Own

- the PTY or direct PTY writes
- session lifecycle policy
- any `ATM_HOME` reference or ATM Rust crate dependency
- any behavior required for non-ATM session use

## Product Requirements Satisfied

The following product requirements from `../requirements.md` are implemented
by this crate:

- Sprint 2 ATM Integration Requirements section — ATM adapter-only items
- ATM Message Relevance Filter section
- Hard Constraints section (ATM boundary rules)
- Inbound Message Delivery section
- Message Format section
- Failure Handling section

## REQ-TERM-ATM-001 — No ATM Rust Crate Dependencies

This crate shall not depend on `agent-team-mail-*` or any ATM Rust crates.
It integrates only via the external `atm` CLI.

Satisfies: Hard Constraints section in `../requirements.md` and RULE-001 in
`../../.github/workflows/ci.yml` ATM boundary gate.

## REQ-TERM-ATM-002 — No ATM_HOME Reference

This crate shall not read, set, or reference the `ATM_HOME` environment
variable.

Satisfies: Hard Constraints section in `../requirements.md`.

## REQ-TERM-ATM-003 — Relevance Filter Is Independently Testable

The inbound message relevance filter shall be testable without a live ATM
installation.

Satisfies: ATM Message Relevance Filter section in `../requirements.md`.

## REQ-TERM-ATM-004 — Exactly-Once Delivery Per Session

Each inbound ATM message shall be injected at most once per session. Watcher
restarts and client reconnects shall not cause reinjection.

Satisfies: Inbound Message Delivery section and Failure Handling section in
`../requirements.md`.

## REQ-TERM-ATM-005 — Blocking Read and Relevance Ownership

`scterm-atm` shall own the adapter mechanics that make inbound ATM delivery
usable without leaking ATM concerns elsewhere:

- blocking `atm read --timeout ...` or equivalent subscription usage
- the relevance filter and self-sender suppression rules
- sender/text normalization before the app layer sees an inbound event

The app layer consumes normalized injection requests; it does not parse ATM CLI
output itself.

Satisfies: ATM Message Relevance Filter section, Hard Constraints section, and
Inbound Message Delivery section in `../requirements.md`.

## REQ-TERM-ATM-006 — Failure Isolation

ATM adapter failures shall degrade gracefully:

- missing `atm` CLI
- malformed ATM output
- watcher crash or restart
- dedupe-state persistence failures

These failures must not become mandatory runtime dependencies for non-ATM
session use.

Satisfies: Failure Handling section in `../requirements.md`.
