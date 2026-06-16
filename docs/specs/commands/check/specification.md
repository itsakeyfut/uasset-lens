# `check` Command — Specification

## Purpose

Run all (or a selected subset of) health checks in one pass and exit with a code that
CI pipelines can gate on. Aggregates the results of `dead-assets`, `cycles`,
`redirectors`, `lint`, `budget`, and `duplicates` into a single report.

```bash
uasset-lens check ./Project
```

Requires `uasset-lens scan` to have been run first.

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | All checks pass — no problems found |
| `1` | One or more checks found problems |
| `2` | Execution error (I/O failure, DB missing, parse error) |

---

## Available Checks

| Check name | What it detects |
|---|---|
| `dead-assets` | Assets not referenced by any other asset |
| `cycles` | Circular dependency cycles in the dependency graph |
| `redirectors` | ObjectRedirector assets in the project |
| `lint` | Lint rule violations (naming conventions, Blueprint complexity, etc.) |
| `budget` | Assets exceeding per-type size budgets configured in `.uasset-lens.toml` |
| `duplicates` | Same-name and texture duplicate asset groups |

All six checks are enabled by default. Use `--only` or `--skip` to control which run.

---

## Text Output

```
$ uasset-lens check ./Project

[dead-assets] 3 unreferenced assets found
  /Game/Unused/T_OldRock (Texture2D, 2.1 MB)
  /Game/Unused/SM_Barrel (StaticMesh, 4.8 MB)
  /Game/Unused/BP_OldEnemy (Blueprint, 0.2 MB)
  ... (3 shown)

[cycles] 1 circular dependency found
  BP_Player → BP_Enemy → BP_GameMode → BP_Player

[redirectors] no redirectors found

[lint] 2 violations found
  /Game/Meshes/Rock.uasset: StaticMesh missing required prefix 'SM_'
  /Game/Characters/BP_Player.uasset: EventTick node count (8) exceeds limit (5)
  ... (2 shown)

[budget] no budget violations

[duplicates] no duplicates found

check failed: dead-assets(3), cycles(1), lint(2)
```

All-pass output:

```
check passed: all 6 checks clean (1,024 assets)
```

With `--verbose`, full findings are printed instead of the first 5 per check:

```
[dead-assets] 47 unreferenced assets found
  /Game/Unused/T_OldRock (Texture2D, 2.1 MB)
  ... (all 47 listed)
```

---

## JSON Output (`--format json`)

```json
{
  "passed": false,
  "checks": {
    "dead-assets":  { "passed": false, "count": 3 },
    "cycles":       { "passed": false, "count": 1 },
    "redirectors":  { "passed": true,  "count": 0 },
    "lint":         { "passed": false, "count": 2 },
    "budget":       { "passed": true,  "count": 0 },
    "duplicates":   { "passed": true,  "count": 0 }
  }
}
```

---

## Selective Checks

```bash
# Run only cycle and lint checks
uasset-lens check ./Project --only cycles,lint

# Run all except dead-assets
uasset-lens check ./Project --skip dead-assets

# Run all except dead-assets and duplicates
uasset-lens check ./Project --skip dead-assets,duplicates
```

`--only` and `--skip` are mutually exclusive.

---

## CI Usage

```yaml
- name: Scan assets
  run: uasset-lens scan ./Project -y

- name: Health check
  run: uasset-lens check ./Project --format github-actions
```

With `--format github-actions`, lint and budget violations are emitted as inline PR
annotations. Dead assets, cycles, redirectors, and duplicates are emitted as notices.
