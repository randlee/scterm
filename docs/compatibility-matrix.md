# scterm Compatibility Matrix

## Purpose

This document maps `atch` compatibility requirements to target ownership and
tests so Sprint 1 does not drift into “similar but different” behavior.

Source-file note:

- `atch.c` is the main CLI entry point
- `attach.c` is the client-side attach/detach implementation
- `master.c` is the master/server loop

## Matrix

| Behavior | `atch` source of truth | Target owner | Primary test layer |
|---|---|---|---|
| default open attach-or-create | `atch.c`, `tests/test.sh` | `scterm-app` | compatibility |
| strict attach | `atch.c`, `tests/test.sh` | `scterm-app` | compatibility |
| `new`, `start`, `run` semantics | `atch.c`, `tests/test.sh` | `scterm-app` | compatibility |
| `push` semantics | `attach.c`, `tests/test.sh` | `scterm-app` + `scterm-unix` | compatibility |
| `kill` grace and force modes | `attach.c`, `tests/test.sh` | `scterm-app` + `scterm-unix` | compatibility |
| `clear` semantics | `atch.c`, `tests/test.sh` | `scterm-app` | compatibility |
| `current` ancestry rendering | `atch.c`, `tests/test.sh` | `scterm-core` + `scterm-app` | unit + compatibility |
| session path expansion | `atch.c`, `tests/test.sh` | `scterm-core` | unit |
| `$HOME` fallback rules | `atch.c`, `tests/test.sh` | `scterm-core` | unit + compatibility |
| no-TTY failures | `atch.c`, `tests/test.sh` | `scterm-app` + `scterm-unix` | compatibility |
| detach-char behavior | `attach.c` | `scterm-app` + `scterm-unix` | Unix integration |
| suspend behavior | `attach.c` | `scterm-app` + `scterm-unix` | Unix integration |
| SIGWINCH forwarding | `attach.c`, `master.c` | `scterm-unix` + `scterm-app` | Unix integration |
| client->master packet protocol | `atch.h`, `attach.c`, `master.c` | `scterm-core` | unit |
| master->client raw byte stream | `atch.h`, `attach.c`, `master.c` | `scterm-app` + `scterm-unix` | Unix integration |
| log replay before live attach | `attach.c` | `scterm-app` | Unix integration |
| ring replay skipping after log replay | `attach.c`, `master.c` | `scterm-app` | Unix integration |
| persistent on-disk log cap | `config.h`, `master.c`, `tests/test.sh` | `scterm-core` + `scterm-app` | unit + compatibility |
| multi-client attach | `master.c` | `scterm-app` + `scterm-unix` | Unix integration |
| stale socket handling | `attach.c`, `tests/test.sh` | `scterm-app` + `scterm-unix` | compatibility |
| ancestry env-var derivation from binary name | `atch.c`, `tests/test.sh` | `scterm-core` | unit + compatibility |
| self-attach prevention | `attach.c` | `scterm-core` + `scterm-app` | unit + compatibility |

## Compatibility Rule

When behavior differs between a research note and the `atch` source/tests,
`atch` source plus `tests/test.sh` wins.
