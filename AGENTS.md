# AGENTS Instructions for scterm

## Must Read

Before participating in team work for this repo, read:
- `docs/team-protocol.md`

## Quick Rule

Always follow this sequence for every ATM message:
1. Immediate acknowledgement
2. Do the work
3. Completion summary
4. Immediate completion acknowledgement by receiver

No silent processing.

---

## Project Overview

`scterm` is a terminal interceptor with no ATM runtime dependencies.

## Key Documents

- [`docs/requirements.md`](docs/requirements.md) — product requirements
- [`docs/architecture.md`](docs/architecture.md) — architecture and boundaries
- [`docs/cross-platform-guidelines.md`](docs/cross-platform-guidelines.md) — portability rules
- [`.claude/skills/rust-development/guidelines.txt`](.claude/skills/rust-development/guidelines.txt) — Rust coding guidelines (read before writing code)

## Build and Test

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all
```

## Hard Boundaries

Violations are blocking — no exceptions:

- No `agent-team-mail-*` crate in any `Cargo.toml`
- No `ATM_HOME` env var referenced anywhere in source
- No `use agent_team_mail::` or `use atm_*::` imports
- Any ATM integration belongs in ATM, not here

## Agent Startup Files

- `pm/arch-term.md` — arch-term (lead, Claude)
- `pm/arch-cterm.md` — cterm (developer, Codex)
