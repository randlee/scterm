---
name: sprint-report
description: Generate a sprint status report for the current phase. Default is --table.
---

# Sprint Report Skill

Build fenced JSON and pipe to the Jinja2 template. `mode` controls table vs detailed.

## Usage

```
/sprint-report [--table | --detailed]
```

Default: `--table`

---

## Data Source

**Always use `atm gh pr list` first** — single call, returns all open PRs with CI and merge state:

```bash
atm gh pr list
```

This is faster and sufficient for populating `sprint_rows` and `integration_row`. Only drill into individual `gh run view` calls if you need failure details for a specific job.

**Dogfooding rule**: If `atm gh pr list` output is missing information needed to fill the report (e.g., no per-job failure detail, no QA state, truncated CI summary), **file a GitHub issue** describing what field or format change would make it sufficient, then improve the command. Do not silently work around gaps with extra `gh` CLI calls — surface them as product issues.

## Render Command

The template path is relative — must run from the **main repo root** (not a worktree).

```bash
cd "${CLAUDE_PROJECT_DIR:-$(git worktree list | head -1 | awk '{print $1}')}"
echo '<json>' > /tmp/sprint-report.json
sc-compose render .claude/skills/sprint-report/report.md.j2 --var-file /tmp/sprint-report.json
```

## --table (default)

```json
{
  "mode": "table",
  "sprint_rows": "| phase-0 | ✅ | ✅ | 🏁 | #3 |\n| phase-1 | ✅ | ✅ | 🚀 | #4 |",
  "integration_row": "| **develop** | | — | 🌀 | — |"
}
```

## --detailed

```json
{
  "mode": "detailed",
  "sprint_rows": "Sprint: phase-0  workspace skeleton\nPR: #3\nQA: PASS ✓ (iter 2)\nCI: Merged to develop ✓\n────────────────────────────────────────\nSprint: phase-1  scterm-core\nPR: #4\nQA: PASS ✓\nCI: Ready to merge",
  "integration_row": "Integration: develop\nCI: Awaiting phase-3+ PRs"
}
```

## Icon Reference

| State | DEV | QA | CI |
|-------|-----|----|----|
| Assigned | 📥 | 📥 | |
| In progress | 🌀 | 🌀 | 🌀 |
| Done/Pass | ✅ | ✅ | ✅ |
| Findings | 🚩 | 🚩 | |
| Fixing | 🔨 | | |
| Blocked | | | 🚧 |
| Fail | | | ❌ |
| Merged | | | 🏁 |
| Ready to merge | | | 🚀 |
