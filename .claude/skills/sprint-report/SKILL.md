---
name: sprint-report
description: Generate a phase status report for scterm. Default is --table.
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

Use `gh pr list` to get current phase PR state:

```bash
cd /Users/randlee/Documents/github/scterm
gh pr list --state all --limit 20
```

For CI status on open PRs:

```bash
gh pr checks <PR_NUMBER>
```

Only drill into individual `gh run view` calls if you need failure details for a specific job.

## Render Command

The template path is relative — must run from the **scterm repo root** (not a worktree).

```bash
cd /Users/randlee/Documents/github/scterm
echo '<json>' > /tmp/sprint-report.json
sc-compose render skills/sprint-report/report.md.j2 --var-file /tmp/sprint-report.json
```

## --table (default)

```json
{
  "mode": "table",
  "sprint_rows": "| Phase 0 Workspace skeleton | ✅ | ✅ | 🏁 | #3 |\n| Phase 1 scterm-core | ✅ | ✅ | 🌀 | #4 |",
  "integration_row": "| **develop integration** | | — | 🌀 | — |"
}
```

## --detailed

```json
{
  "mode": "detailed",
  "sprint_rows": "Phase: 0  Workspace skeleton\nPR: #3\nQA: PASS ✓\nCI: Merged to develop ✓\n────────────────────────────────────────\nPhase: 1  scterm-core\nPR: #4\nQA: PASS ✓\nCI: Running (1 pending)",
  "integration_row": "Integration: develop\nCI: Pending phase-2 merge"
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
