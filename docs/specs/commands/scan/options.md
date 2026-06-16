# `scan` Command — Options

## Synopsis

```
uasset-lens scan <project_dir> [options]
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root or Content directory |

---

## Options

### `--full-scan`

Re-scan all files regardless of modification time.

By default, only files whose `mtime` has changed since the last scan are re-parsed.
Use `--full-scan` when the DB may be out of sync despite `mtime` being unchanged
(e.g., after restoring files from version control).

```bash
uasset-lens scan ./Project --full-scan
```

---

### `--diff`

Show a diff of changes compared to the previous scan.

Prints added, updated, and removed assets after scanning. Conflicts with `--diff-from`.

```bash
uasset-lens scan ./Project --diff
```

---

### `--save-baseline <NAME>`

Save the scan result as a named baseline for later `--diff-from` comparisons.

Baselines are stored in `.uasset-lens/baselines/<NAME>` within the project directory.

```bash
uasset-lens scan ./Project --save-baseline main
uasset-lens scan ./Project --save-baseline before-refactor
```

---

### `--diff-from <NAME>`

Diff against a named baseline instead of the previous scan. Implies `--diff`.

Conflicts with `--diff`.

```bash
uasset-lens scan ./Project --diff-from main
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts (auto-removes stale records) |
