# scterm State Machines

## Purpose

This document records the allowed lifecycle transitions for Sprint 1 and the
ATM extension.

## Session Master Lifecycle

```text
Resolved -> Starting -> Running -> Exiting -> Exited
```

Rules:

- only `Resolved` may transition to `Starting`
- only `Starting` may transition to `Running` or startup failure
- only `Running` owns a live PTY and control socket
- `Exited` is terminal

## Attach Client Lifecycle

```text
LogReplaying -> Connecting -> RingReplaying -> Live
Connecting -> Live
Live -> Suspended -> Live
Live -> Detached
Live -> Exited
```

Rules:

- log replay precedes socket connection because it reads the on-disk file directly
- log replay may be skipped when no log exists
- ring replay is a subphase between socket connect and live attach
- `Detached` and `Exited` are terminal for the client instance

## Client Attachment State

```text
Connected + Unattached -> Replaying -> Attached
Attached -> Unattached
```

This is master-side per-client state, not the whole session state.

## ATM Injection Lifecycle

```text
Observed -> Normalized -> DedupChecked -> Queued -> Injected
Observed -> Dropped
Normalized -> Dropped
DedupChecked -> Dropped
Queued -> Dropped
```

Rules:

- only normalized messages may be queued
- only queued messages may be injected
- duplicate detection happens before queueing
- dropped messages do not affect session liveness
