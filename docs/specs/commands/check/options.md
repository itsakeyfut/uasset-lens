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

### `--format <FORMAT>`

Output format.

| Value | Description |
|---|---|
| `text` | Human-readable text output (default) |
| `json` | Structured JSON to stdout — one object, no streaming |
| `github-actions` | GitHub Actions annotation syntax for inline PR comments |

Default: `text`

```bash
uasset-lens check ./Project --format github-actions
uasset-lens check ./Project --format json | jq '.summary'
```

---

### `--save-baseline [PATH]`

Save the current check results as a baseline JSON file.

- Default path: value of `check.baseline_path` in `.uasset-lens.toml`, or
  `.uasset-lens/baseline.json` if not set.
- **Always exits `0`** unless there is an execution error (exit `2`).
- Intended for the main branch CI job that sets the reference point.
- The file includes `git_commit` (from `git rev-parse HEAD` if available).

```bash
uasset-lens check ./Project --save-baseline
uasset-lens check ./Project --save-baseline ./ci/baseline.json
```

---

### `--diff-from <PATH>`

Compare current results to a previously saved baseline. Fails only on regressions
(violations that appear in the current run but were absent in the baseline).

- Matching is by `rule` + `asset_path`.
- Violations present in both → not a regression.
- Violations in current but not in baseline → regression → exit `1`.
- Violations in baseline but not in current → resolved → exit `0`.

```bash
uasset-lens check ./Project --diff-from .uasset-lens/baseline.json
```

Can be combined with `--format`:

```bash
uasset-lens check ./Project \
  --diff-from .uasset-lens/baseline.json \
  --format github-actions
```

---

### `--skip-scan`

Skip the mtime delta scan step and use the existing DB as-is.

Use when:
- A previous CI step already ran `uasset-lens scan`
- The DB is known to be up-to-date

```bash
uasset-lens scan ./Project -y
uasset-lens check ./Project --skip-scan --format github-actions
```

---

### `-y` / `--yes`

Non-interactive mode. Suppresses all confirmation prompts. Required for CI.

```bash
uasset-lens check ./Project -y
```

---

### `--db <PATH>`

Override the DB file path. Default: `<project_dir>/.uasset-lens/uasset-lens.db`

```bash
uasset-lens check ./Project --db /tmp/lens.db
```

---

## Common Flag Combinations

```bash
# Minimal CI gate
uasset-lens check ./Project -y

# PR annotation with regression detection
uasset-lens check ./Project \
  --diff-from .uasset-lens/baseline.json \
  --format github-actions \
  -y

# Save baseline (run on main branch)
uasset-lens check ./Project --save-baseline -y

# JSON output for downstream processing
uasset-lens check ./Project --format json -y
```
