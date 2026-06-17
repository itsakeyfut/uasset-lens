# `check` Command — Options

## Synopsis

```
uasset-lens check <project_dir> [options]
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root or Content directory |

---

## Options

### `--only <CHECKS>`

Run only the specified checks. Comma-separated list of check names.

Valid values: `dead-assets`, `cycles`, `redirectors`, `lint`, `budget`, `duplicates`

Mutually exclusive with `--skip`.

```bash
uasset-lens check ./Project --only cycles,lint
uasset-lens check ./Project --only budget
```

---

### `--skip <CHECKS>`

Skip the specified checks. Comma-separated list of check names.

Valid values: `dead-assets`, `cycles`, `redirectors`, `lint`, `budget`, `duplicates`

Mutually exclusive with `--only`.

```bash
uasset-lens check ./Project --skip dead-assets
uasset-lens check ./Project --skip dead-assets,duplicates
```

---

### `--verbose`

Show all findings for each check instead of the first 5.

By default, each check shows at most 5 items followed by a `... (N more)` summary line.
With `--verbose`, all items are printed.

The 5-item cap applies to the default `text` format only. `--format json` and
`--format github-actions` always include the full results regardless of `--verbose`.

```bash
uasset-lens check ./Project --verbose
```

---

### `--fail-on <LEVEL>`

Exit `1` if violations of the given severity or higher are found.

| Value | Exit 1 condition |
|---|---|
| `error` | One or more error-severity violations (default) |
| `warn` | One or more violations of any severity (errors or warnings) |
| `never` | Never exit 1 (informational only) |

```bash
uasset-lens check ./Project --fail-on warn   # fail on any warning
uasset-lens check ./Project --fail-on never  # always exit 0 (for dashboards)
```

`--fail-on warn` is intended for stricter CI gates where warnings must be resolved.

---

### `--skip-scan`

Use the existing DB without running a scan first.

By default, `check` runs an mtime delta scan before evaluating rules. Use `--skip-scan`
when the DB is guaranteed up-to-date (e.g., in a pipeline that ran `scan` in a prior step).

```bash
uasset-lens check ./Project --skip-scan
```

---

### `--save-baseline [PATH]`

Save the current check results as a violation baseline JSON file.

If `PATH` is omitted, saves to `.uasset-lens/baselines/check-baseline.json`.

```bash
uasset-lens check ./Project --save-baseline
uasset-lens check ./Project --save-baseline .uasset-lens/baselines/main.json
```

---

### `--diff-from <PATH>`

Compare current results against a previously saved baseline. Exit `1` only if new
regressions are found (violations in current that were not in the baseline).

```bash
uasset-lens check ./Project --diff-from .uasset-lens/baselines/main.json
```

Violations present in both the baseline and the current run are not reported as failures.
Warnings are excluded from regression detection.

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions\|sarif>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts |
