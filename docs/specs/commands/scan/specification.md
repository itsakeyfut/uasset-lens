# `scan` Command — Specification

## Purpose

Scan all `.uasset` and `.umap` files under the Content directory and index asset
metadata and dependencies into the SQLite database. Must be run before any other
command can operate.

```bash
uasset-lens scan ./Project
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Scan completed, no DB records removed |
| `1` | One or more stale DB records were removed (files no longer on disk) |
| `2` | Execution error (I/O failure, permission denied, parse error) |

---

## Scan Strategy

By default, only files whose `mtime` has changed since the last scan are re-parsed
(delta scan). Use `--full-scan` to force re-parsing of all files regardless of `mtime`.

Files that fail to parse are skipped and reported as warnings. They do not cause exit `2`.

---

## Stale Record Handling

When files have been deleted from disk since the last scan, `scan` detects the stale
records and prompts for confirmation before removing them from the DB.

With `-y` / `--yes`, stale records are removed without prompting (required for CI).

```
The following DB records have no corresponding file on disk:
  /Game/Old/BP_Deprecated
  /Game/Temp/M_Test
Remove these records from DB? [y/N]: y
```

---

## Text Output

```
$ uasset-lens scan ./Project

Scanning ./Project/Content... (1,024 files)
  + 3 new assets indexed
  ~ 5 assets updated (mtime changed)
  ? 2 assets removed from disk

The following DB records have no corresponding file on disk:
  /Game/Old/BP_Deprecated
  /Game/Temp/M_Test
Remove these records from DB? [y/N]: y

✓ 1,022 assets total, 2 records cleaned

Skipped (2 parse errors):
  WARN Content/Broken/BP_X.uasset: invalid magic number
  WARN Content/Old/M_Y.uasset: unsupported version
```

---

## JSON Output (`--format json`)

```json
{
  "assets_total": 1022,
  "new": 3,
  "updated": 5,
  "removed": 2,
  "skipped": [
    { "path": "Content/Broken/BP_X.uasset", "reason": "invalid magic number" },
    { "path": "Content/Old/M_Y.uasset",     "reason": "unsupported version" }
  ]
}
```

---

## Diff Output (`--diff`)

Shows a summary of what changed relative to the previous scan.

```
$ uasset-lens scan ./Project --diff

Scanning... (1,024 files)

Changes since last scan:
  + /Game/Characters/BP_NewEnemy (Blueprint, 0.3 MB)
  ~ /Game/Materials/M_Rock (Material, 1.1 MB → 1.4 MB)
  - /Game/Unused/T_OldRock (Texture2D, 2.1 MB)
```

---

## Baseline Workflow (`--save-baseline` / `--diff-from`)

Named baselines allow comparing the current scan against a known-good state.

```bash
# Save the current scan as a named baseline
uasset-lens scan ./Project --save-baseline main

# Show diff against the saved baseline
uasset-lens scan ./Project --diff-from main
```

Baselines are stored in `.uasset-lens/baselines/` within the project directory.
`--diff-from` implies `--diff` automatically.
