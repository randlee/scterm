# Claude Instructions for scterm

## Critical Workflow Rule

Do not switch the main checkout away from `main` for sprint work.

- Keep the primary repo checkout on `main`
- Use git worktrees for feature work when parallel branches are needed
- Prefer short-lived feature branches targeting `develop`

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

## Team Communication

If this repo is being run with ATM team workflow enabled, follow
[`docs/team-protocol.md`](./docs/team-protocol.md) for all ATM messages.
