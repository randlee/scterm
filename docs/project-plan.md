# scterm Project Plan

## Completed Sprints

| Sprint | Scope | Status |
|--------|-------|--------|
| Sprint 1 | atch behavioral parity (Phases 0–5) | ✅ Done |
| Sprint 2 | ATM message integration (Phase 6) | ✅ Done |
| sc-observability-logging | Replace self-contained AppLogger with sc-observability backend | ✅ QA PASS — PR #20 pending merge |

## Open Tasks

| ID | Task | Blocked by |
|----|------|-----------|
| TASK-001 | Merge PR #20 (sc-observability-logging → develop) | CI red: sc-observability path dep not on CI runner. Merge with known cause or wait for publish. |
| TASK-002 | Publish sc-observability to crates.io + swap path dep to version pin | External: sc-observability repo publish workflow |
| TASK-003 | Merge PR #19 (develop → main) | 1 approving review required |
| TASK-004 | docs/crate-boundaries.md — add sc-observability/sc-observability-types to scterm-app "May depend on" list | Batch with next doc sprint (REQ-QA-008, non-blocking) |
| TASK-005 | versioning-publish-standards worktree — commit PR or discard | User decision |

## Future Sprints

_Planned work to be defined here before sprint start._
