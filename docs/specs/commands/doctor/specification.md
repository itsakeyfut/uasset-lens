# `doctor` Command — Specification

## Purpose

Check the health of the local uasset-lens installation: tool version, database
existence and schema version, config file validity, last scan freshness, and
scanner/DB compatibility. Intended as a first diagnostic step when something
appears wrong.

```bash
uasset-lens doctor ./Project
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | All checks passed |
| `1` | One or more checks failed |
| `2` | Execution error (I/O failure reading the DB file) |

---

## Checks

| Check | What is verified |
|---|---|
| `[DB]` | `.uasset-lens/uasset-lens.db` exists and can be opened |
| `[Schema]` | DB schema version matches `CURRENT_SCHEMA_VERSION` |
| `[Config]` | `.uasset-lens.toml` parses without errors (skipped if file is absent) |
| `[Scan]` | At least one scan has been recorded; reports days since last scan |
| `[Compat]` | Scanner version embedded in DB matches the current binary version |

Checks that cannot run because a prerequisite failed are shown with `—` (not applicable).

---

## Text Output

All five checks are always printed, each with `✓` (pass), `✗` (fail), or `—` (skipped because a
prerequisite failed). All-pass example:

```
uasset-lens v0.2.0

[DB]     ✓  .uasset-lens/uasset-lens.db (1,024 assets)
[Schema] ✓  v4 (current)
[Config] ✓  .uasset-lens.toml (valid)
[Scan]   ✓  Last scan: 2026-06-16 14:23:00 UTC (0 days ago)
[Compat] ✓  Scanner version v0.2.0 matches binary

All checks passed.
```

With issues — when the DB is missing, the schema/scan/compat checks depend on it and are skipped
(`—`), so only the `[DB]` failure counts:

```
uasset-lens v0.2.0

[DB]     ✗  .uasset-lens/uasset-lens.db not found
             Run 'uasset-lens scan <project_dir>' to create it.
[Schema] —  Cannot check (DB missing)
[Config] ✓  .uasset-lens.toml (valid)
[Scan]   —  Cannot check (DB missing)
[Compat] —  Cannot check (DB missing)

1 issue found.
```

Schema mismatch (DB opens, but its version and scanner version differ from the binary):

```
uasset-lens v0.2.0

[DB]     ✓  .uasset-lens/uasset-lens.db (512 assets)
[Schema] ✗  v3 is outdated (expected v4)
             Run 'uasset-lens scan --full-scan <project_dir>' to migrate.
[Config] ✓  .uasset-lens.toml (valid)
[Scan]   ✓  Last scan: 2026-06-14 09:10:00 UTC (2 days ago)
[Compat] ✗  Scanner version mismatch: DB built with v0.1.3, current binary is v0.2.0

2 issues found.
```

Config absent (not an error — defaults are used):

```
[Config] —  .uasset-lens.toml not found (defaults will be used)
```

---

## JSON Output (`--format json`)

```json
{
  "tool_version": "0.2.0",
  "checks": {
    "db": {
      "passed": true,
      "schema_version": 4,
      "asset_count": 1024
    },
    "schema": {
      "passed": true,
      "skipped": false,
      "version": 4,
      "expected": 4
    },
    "config": {
      "passed": true,
      "present": true
    },
    "scan": {
      "passed": true,
      "skipped": false,
      "last_scan_utc": "2026-06-16T14:23:00Z",
      "days_since_scan": 0
    },
    "compat": {
      "passed": true,
      "skipped": false,
      "db_scanner_version": "0.2.0",
      "binary_version": "0.2.0"
    }
  },
  "issues_found": 0
}
```

When the DB is missing, `"db"` reports `"passed": false` with `"schema_version"` and
`"asset_count"` null, and the dependent `"schema"`, `"scan"`, and `"compat"` checks each report
`"passed": false` with `"skipped": true` (a skipped check never increments `issues_found`).

---

## Staleness Threshold

The `[Scan]` check does not fail based on staleness alone — it only reports how long ago
the last scan was run. Staleness is informational; the exit code is `1` only when the DB
is missing or no scan has ever been recorded.
