# scterm

**AI-native terminal multiplexer for running and steering large teams of background agents.**

`scterm` is a terminal session interceptor built for the era of agentic AI workflows. It wraps any CLI process — a coding agent, a shell, a long-running pipeline — in a persistent, attachable session with reliable cross-terminal messaging baked in. Detach, reattach, monitor, and steer dozens of background agents from a single seat.

## Why scterm

Modern AI CLI tools like Claude Code, Codex, and Gemini run long, autonomous sessions. Coordinating them across many terminals is awkward: sessions die when you disconnect, there is no standard way to send them new instructions mid-run, and tracking which agent is doing what requires manual bookkeeping.

`scterm` solves this by giving each agent session a persistent socket, a scrollback history, and a message injection path — so you can leave agents running, check in on any of them, and route new work to them without interrupting their current task.

## Features

- **Attach / detach** — leave any agent session running in the background and reattach from any terminal, exactly where it left off
- **Scrollback replay** — full session history replayed on attach; both a fast in-memory ring and a persistent on-disk log survive disconnects and reboots
- **Cross-terminal messaging** — inject text input into any running session from another terminal; compatible with the `atm` (agent-team-mail) message bus for reliable inter-agent communication
- **Nested session awareness** — tracks session ancestry so agents know which sessions they are nested inside; prevents accidental self-attach loops
- **Multi-client** — multiple terminals can observe the same session simultaneously
- **atch-compatible** — drop-in compatible with `atch` workflows and scripts; existing attach/detach muscle memory carries over
- **Minimal footprint** — no terminal emulation, no reimplemented PTY protocol; raw output passthrough preserves full terminal compatibility with any CLI tool

## Use Cases

- Run a team of AI coding agents (Claude Code, Codex, Gemini, etc.) in persistent background sessions
- Steer a running agent by injecting a new prompt from another terminal without interrupting its current context
- Monitor many agent sessions from a single "control" terminal using `list` and attach
- Route ATM (agent-team-mail) messages directly into the agent's stdin so idle agents wake and process new work automatically
- Recover gracefully from stale sessions after a crash or power loss without losing log history

## Status

Sprint 1 in progress — core `atch`-compatible terminal attach/detach engine.
Sprint 2 adds the ATM message bridge for cross-agent messaging.

## Architecture

Four crates:

| Crate | Role |
|-------|------|
| `scterm-core` | Session model, packet types, ring buffer, validated domain types, error surface |
| `scterm-unix` | PTY, Unix sockets, raw-mode terminal, signals, daemonization |
| `scterm-app` | Session orchestration, CLI, structured logging, PTY write serialization |
| `scterm-atm` | ATM message adapter — blocking inbound receive, dedupe, PTY injection (Sprint 2) |

See [`docs/architecture.md`](docs/architecture.md) for the full design.

## Build

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings
```

Requires a stable Rust toolchain. Targets macOS and Linux.
