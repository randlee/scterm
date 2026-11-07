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

## Honest Work, No Ceremony (binding on agents and humans)

Goal: working, deployable capability. Process serves it and never becomes the product.

- **Process artifacts** (certificates, ledgers, dashboards, matrices, meta-reports, speculative checks) need four things to exist: a named consumer, the feature it gates, the observed defect class that justifies it, and a deletion condition. Missing any of these → don't create it. Test: if running code branches on it, it's product. If only humans or reports read it, it's process and needs the four things. Writing code just to flip that answer is itself the pathology. Exception: a minimal integrity/recovery control (crash-recovery state, provenance snapshot) that prevents a named evidence-loss or corruption mode.
- **Real code + real tests in the same unit of work.** Forbidden: faked tests, mocks/fixtures presented as live proof, weakened assertions, regenerating goldens to force green, hard-coded success paths, placeholder macros in commits, editing the spec instead of implementing it, narrowing scope while claiming full success.
- **No self-certification.** An independent verifier closes the work, citing evidence at an exact revision. In solo sessions, re-verify by re-running, and state what wasn't independently verified.
- **Typed refusal beats fabrication but is worth less than real capability.** Work that ends only in a refusal stays open and says so.
- **Null results are successes** ("checked X, found no material increment"). Unsupported claims are worse than silence.
- **Metrics** predeclare their denominator and countermetric. Agreement between agents can raise confidence but is never independent evidence. Never silence stderr in commands that produce evidence.
- **Name pathologies when they occur:** gate self-weakening, proof-class inflation, golden regeneration, tolerance widening, suppression-pragma laundering, refusal farming, follow-up laundering. The full catalog and countermeasures are in the `just-say-no-to-process-porn-and-ceremony` skill. Ask the operator for it if you can't find it.
