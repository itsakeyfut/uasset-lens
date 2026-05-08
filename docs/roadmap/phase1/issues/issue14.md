# `crates/cli` — `scan` command: deletion detection, output, and exit codes

## Summary

Add stale-asset detection to `scan`, implement the full text and JSON output formats,
and wire up correct exit codes.
Complete when deleted assets trigger a `[y/N]` prompt and `--format json` emits a
JSON summary matching the spec.

## Design Notes

**Stale asset detection:**

Before the scan begins, call `db.all_known_files()` to snapshot the set of indexed paths.
After walkdir enumeration, compute the set difference: paths in DB but not found on disk.

**Confirmation prompt:**

```
3 assets no longer exist on disk. Remove from index? [y/N]
```

- Read answer from stdin (default = No on Enter)
- If `-y` / `--yes` flag is set: skip prompt and remove automatically
- If user answers `N` or presses Enter: leave stale assets in DB (exit code 0)

**JSON output schema (from `docs/specs/cli-design.md`):**

```json
{
  "scanned":     42,
  "indexed":     40,
  "skipped":     2,
  "removed":     3,
  "duration_ms": 312
}
```

**Exit codes:**

| Code | Condition |
|---|---|
| 0 | Scan completed, no removals |
| 1 | One or more stale assets removed from index |
| 2 | Execution error (IO, DB, parse failure) |

**ANSI color:** check `std::io::stdout().is_terminal()` and `NO_COLOR` env var before emitting
any color codes. Text output to stderr must never include ANSI codes regardless.

## Requirements

- [ ] Snapshot `db.all_known_files()` before walkdir runs
- [ ] Compute stale paths: `db_files - walkdir_files`
- [ ] Print confirmation prompt when stale paths found and `-y` not set
- [ ] Read user confirmation from stdin; treat empty input as `N`
- [ ] Call `db.delete_asset()` for confirmed stale paths
- [ ] Implement text output format matching `docs/rules/cli-output.md`
- [ ] Implement JSON output for `--format json` matching the scan JSON schema
- [ ] Disable ANSI color when stdout is not a TTY or `NO_COLOR` is set
- [ ] Exit code 0 when nothing removed, 1 when stale assets removed, 2 on error

## Related

- Depends on: #13 (scan core)
- Docs: `docs/specs/cli-design.md` (scan JSON schema), `docs/rules/cli-output.md`
