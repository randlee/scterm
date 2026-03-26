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

CI (`.github/workflows/ci.yml`) runs on every PR and push to `main`/`develop`:
- `fmt` — format check
- `clippy` — lint with `-D warnings`
- `atm-boundary` — grep scan blocking any ATM crate deps, ATM_HOME refs, or ATM imports
- `test` — build + test on ubuntu-latest and macos-latest

All 5 checks are required gates on both branches.

## Hard Boundaries

Violations are blocking — no exceptions:

- No `agent-team-mail-*` crate in any `Cargo.toml`
- No `ATM_HOME` env var referenced anywhere in source
- No `use agent_team_mail::` or `use atm_*::` imports
- Any ATM integration belongs in ATM, not here

## Agent Startup Files

- `pm/arch-term.md` — arch-term (lead, Claude)
- `pm/arch-cterm.md` — cterm (developer, Codex)
