# `check` Command — Specification

## Purpose

Single CI entry point. Runs all enabled quality checks against a scanned project and
returns an exit code that CI pipelines can gate on.

```bash
uasset-lens check ./Project
```

The command always re-scans first (mtime delta) so CI always sees the current state of
the repository. Use `--skip-scan` to reuse an existing DB.

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | All checks pass — no `"error"` severity violations |
| `1` | One or more `"error"` severity violations found |
| `2` | Execution error (I/O failure, DB missing, parse error) |

`"warn"` severity violations appear in output but do **not** cause exit `1`.

---

## Text Output

```
$ uasset-lens check ./Project

Scanning... (1,024 assets, 0.8s)
Running checks...

ERRORS (3):
  [blueprint] /Game/Characters/BP_Player.uasset
    EventTick node count (8) exceeds limit (5)
  [budget] /Game/Characters/T_PlayerArmor_D.uasset
    File size (6.1 MB) exceeds Texture2D limit (4 MB)
  [lint] /Game/Meshes/Rock.uasset
    StaticMesh missing required prefix 'SM_'

WARNINGS (12):
  [dead-assets] /Game/Unused/T_OldRock.uasset (Texture2D, 2.1 MB)
  ... (11 more)

check failed: 3 errors, 12 warnings
```

All-pass output:

```
check passed: 0 errors, 0 warnings (1,024 assets, 0.9s)
```

---

## JSON Output (`--format json`)

```json
{
  "summary": {
    "errors": 3,
    "warnings": 12,
    "assets_scanned": 1024,
    "duration_ms": 820
  },
  "violations": [
    {
      "severity": "error",
      "rule": "blueprint.event_tick_limit",
      "asset_path": "/Game/Characters/BP_Player",
      "file": "Content/Characters/BP_Player.uasset",
      "message": "EventTick node count (8) exceeds limit (5)"
    },
    {
      "severity": "warn",
      "rule": "dead-assets",
      "asset_path": "/Game/Unused/T_OldRock",
      "file": "Content/Unused/T_OldRock.uasset",
      "message": "Asset has no incoming references"
    }
  ]
}
```

On execution error (exit `2`): stderr only, no JSON envelope.

---

## GitHub Actions Annotation Output (`--format github-actions`)

Uses [GitHub Actions workflow commands](https://docs.github.com/en/actions/using-workflows/workflow-commands-for-github-actions).
Annotations appear as inline comments in the PR diff view.

Output goes to **stdout** (GitHub Actions reads annotations from stdout).

```
::error file=Content/Characters/BP_Player.uasset,title=blueprint.event_tick_limit::EventTick node count (8) exceeds limit (5)
::error file=Content/Characters/T_PlayerArmor_D.uasset,title=budget.Texture2D::File size (6.1 MB) exceeds limit (4 MB)
::warning file=Content/Unused/T_OldRock.uasset,title=dead-assets::Asset has no incoming references
```

`file` is the path relative to the repository root. `title` is the rule name.

---

## Baseline Comparison (`--diff-from`)

Loads a previously saved baseline and fails only when violations have **increased**.

| Violation state | Outcome |
|---|---|
| Present in both baseline and current | No regression — does not fail |
| Present in current, absent in baseline | New regression — exit `1` |
| Present in baseline, absent in current | Resolved — no fail |

Matching is by `rule` + `asset_path`.

---

## Checks Performed

`check` runs every check enabled in `.uasset-lens.toml`. See `docs/specs/config.md` for
the full configuration reference.

| Config key | What it checks |
|---|---|
| `[rules] dead-assets` | Unreferenced asset detection |
| `[rules] circular-deps` | Circular dependency detection |
| `[rules] duplicate-assets` | Same-name / same-content duplicates |
| `[rules] redirectors` | Unresolved ObjectRedirector assets |
| `[lint] naming.*` | Asset naming convention violations |
| `[lint] blueprint.*` | Blueprint node / EventTick / Cast complexity |
| `[budget] <Type>.*` | Per-type file size budget enforcement |

---

## `check` vs `report`

| | `check` | `report` |
|---|---|---|
| Purpose | CI gate | Human-readable output |
| Exit code | 0 / 1 / 2 (meaningful) | 0 / 2 (always 0 on success) |
| Output | text / json / github-actions | html / markdown |
| Speed | Fast (optimized for CI) | Slower (richer output) |
| Trend data | No | Yes (if baselines present) |

---

## CI Usage Patterns

### Strict gate (greenfield projects)

```yaml
- run: uasset-lens check ./Project --format github-actions
```

Fails on any `"error"` violation.

### Regression-only gate (existing projects with violations)

```yaml
- name: Check assets (regression gate)
  run: |
    uasset-lens check ./Project \
      --diff-from .uasset-lens/baseline.json \
      --format github-actions

- name: Update baseline (main branch only)
  if: github.ref == 'refs/heads/main'
  run: |
    uasset-lens check ./Project --save-baseline
    git add .uasset-lens/baseline.json
    git commit -m "chore: update asset quality baseline" || true
    git push
```
