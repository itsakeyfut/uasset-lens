# `baseline` Command — Specification

## Purpose

Manage named violation baselines for regression detection. A baseline captures the
complete set of lint and budget violations at a point in time. The `check --diff-from`
workflow uses baselines to report only violations that are NEW since the baseline
(regressions), suppressing violations that already existed.

```bash
uasset-lens baseline list ./Project
uasset-lens baseline save ./Project main
uasset-lens baseline delete ./Project before-refactor
uasset-lens baseline diff ./Project main
```

---

## Exit Codes

| Subcommand | Code | Meaning |
|---|---|---|
| `list` | `0` | List displayed (including empty) |
| `list` | `2` | Execution error |
| `save` | `0` | Baseline saved successfully |
| `save` | `2` | Execution error |
| `delete` | `0` | Baseline deleted |
| `delete` | `2` | Baseline not found or I/O error |
| `diff` | `0` | No regressions found |
| `diff` | `1` | One or more regressions found |
| `diff` | `2` | Execution error (baseline not found, DB error) |

---

## Baseline Storage

Baselines are stored as JSON files:

```
<project_dir>/.uasset-lens/baselines/<NAME>.json
```

Schema (version 1):

```json
{
  "version": 1,
  "name": "before-refactor",
  "saved_at": "2026-06-15T09:11:00Z",
  "git_commit": "def5678",
  "summary": { "errors": 3, "warnings": 12 },
  "violations": [
    {
      "rule": "lint/naming/blueprint-prefix",
      "asset_path": "/Game/Characters/Character",
      "severity": "error"
    }
  ]
}
```

`violations` contains every active violation at save time. Each entry is identified by
the pair `(rule, asset_path)`.

---

## `baseline list`

Lists all saved baselines with name, date, error count, and warning count.

```
Baselines (./Project)

Name              Date              Errors  Warnings  Commit
main              2026-06-15 09:11  3       12        def5678
before-refactor   2026-06-14 16:45  5       15        ghi9012
sprint-12         2026-06-01 10:00  2       8         jkl3456

3 baselines
```

When no baselines exist:

```
No baselines saved.
```

---

## `baseline save <NAME>`

Runs all lint and budget checks against the current DB state and saves the results as
a named baseline. Overwrites an existing baseline with the same name after confirmation.

```
$ uasset-lens baseline save ./Project main
Running checks... 1,024 assets
Saved baseline 'main': 3 errors, 12 warnings (def5678)
```

With `--yes`, overwrites without prompting.

---

## `baseline delete <NAME>`

Deletes the named baseline file. Prompts for confirmation unless `--yes` is set.

```
$ uasset-lens baseline delete ./Project before-refactor
Delete baseline 'before-refactor'? [y/N]: y
Deleted baseline 'before-refactor'.
```

If the baseline does not exist, exit `2` with:

```
error: baseline 'before-refactor' not found
```

---

## `baseline diff <NAME>`

Compares current violations against the named baseline.

```
Baseline: main (2026-06-15, 3 errors, 12 warnings)
Current:  now (5 errors, 11 warnings)

Regressions (2 new errors):
  [lint/naming/blueprint-prefix] /Game/Characters/NewCharacter
  [budget/texture2d] /Game/Textures/T_NewRock_D

Resolved (1 error fixed):
  [lint/blueprint/event-tick-count] /Game/Characters/BP_Player
```

When no regressions and no resolutions:

```
No regressions. Baseline 'main' is clean.
```

Regression detection uses set difference on `(rule, asset_path)` pairs.
Only errors contribute to exit code `1`; warnings are reported but do not cause `1`.

---

## JSON Output (`--format json`)

For `baseline diff`:

```json
{
  "baseline": {
    "name": "main",
    "saved_at": "2026-06-15T09:11:00Z",
    "git_commit": "def5678",
    "summary": { "errors": 3, "warnings": 12 }
  },
  "current": {
    "summary": { "errors": 5, "warnings": 11 }
  },
  "regressions": [
    { "rule": "lint/naming/blueprint-prefix", "asset_path": "/Game/Characters/NewCharacter", "severity": "error" }
  ],
  "resolved": [
    { "rule": "lint/blueprint/event-tick-count", "asset_path": "/Game/Characters/BP_Player", "severity": "error" }
  ]
}
```
