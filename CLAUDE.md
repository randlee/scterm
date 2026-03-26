# Claude Instructions for scterm

## Branch Model

This repo uses git-flow with two protected long-lived branches:

- `main` — protected, release-quality only. Never commit directly to main.
- `develop` — protected integration branch. Feature branches target develop.

Sprint work happens on `develop` or short-lived `feature/*` branches that merge into develop via PR. Use git worktrees when parallel branches are needed. Keep the primary repo checkout on `develop`.

PRs into either branch require CI to pass and 1 approving review.

## Project Overview

`scterm` is a terminal interceptor.

This repo is intentionally independent from ATM. Do not introduce
`agent-team-mail-*` dependencies or ATM path/runtime assumptions.

## Key Documents

- [`docs/requirements.md`](./docs/requirements.md)
- [`docs/architecture.md`](./docs/architecture.md)
- [`docs/cross-platform-guidelines.md`](./docs/cross-platform-guidelines.md)
- [`docs/team-protocol.md`](./docs/team-protocol.md)
- [`.claude/skills/rust-development/guidelines.txt`](./.claude/skills/rust-development/guidelines.txt)

## Boundary Rules

1. This repo is fully independent from ATM — no ATM crate dependencies.
2. Do not read `ATM_HOME`.
3. Any ATM integration belongs in ATM adapters, not in this repo.

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
- `test` (ubuntu-latest) — build + test
- `test` (macos-latest) — build + test

## Team Communication

If this repo is being run with ATM team workflow enabled, follow
[`docs/team-protocol.md`](./docs/team-protocol.md) for all ATM messages.
