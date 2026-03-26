# scterm State Machines

## Purpose

This document records the allowed lifecycle transitions for Sprint 1 and the
ATM extension.

## Session Master Lifecycle

```text
Resolved -> Starting -> Running -> Exiting -> Exited
Resolved -> Stale
```

Rules:

- only `Resolved` may transition to `Starting`
- only `Starting` may transition to `Running` or startup failure
- `Resolved -> Stale` occurs only when the socket file exists and `connect()`
  returns `ECONNREFUSED`
- `Stale` is terminal for the detection path; recovery removes the stale socket
  and creates a fresh `Resolved` instance
- `Resolved`, `Running`, and `Stale` are the coarse public typestate states per
  `REQ-RBP-003`; `Starting`, `Exiting`, and `Exited` are internal operational
  phases and need not be public typestate
- `Running` requires all three: control socket created/bound/listening, PTY
  child-start path succeeded, and a fresh client can connect
- only `Running` owns a live PTY and control socket
- `Exited` is terminal

## Attach Client Lifecycle

```text
LogReplaying -> Connecting -> RingReplaying -> Live
Connecting -> Live
Live -> Detached
Live -> Exited
```

Rules:

- log replay precedes socket connection because it reads the on-disk file directly
- log replay may be skipped when no log exists
- ring replay is a subphase between socket connect and live attach
- suspend (`Ctrl-Z` / detach-suspend key) first detaches the client; on process
  resume, the user reattaches via a new `Connecting` cycle rather than
  returning to a distinct `Suspended` state
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
