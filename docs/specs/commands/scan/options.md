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

### `--exclude <PATTERN>`

Exclude files matching a glob pattern in addition to `scan.exclude_paths` from config.
Repeatable. Patterns are matched against the path relative to the project directory
(e.g. `Content/Dev/BP_Tool.uasset`), using the same prefix-or-glob rules as
`scan.exclude_paths`.

```bash
uasset-lens scan ./Project --exclude "Content/Dev/**"
uasset-lens scan ./Project --exclude "Content/**/Test*.uasset" --exclude "Content/QA/"
```

This flag provides a one-off override without modifying `.uasset-lens.toml`.

---

### `--hash`

Use SHA-256 file content hashing in addition to `mtime` to detect changed assets.

By default, `scan` uses `mtime` only (faster). With `--hash`, the SHA-256 digest of each
file is computed and compared to the stored hash. This catches changes when `mtime` is
reset (e.g., after `git checkout`, restoring from backup, or Perforce sync).

```bash
uasset-lens scan ./Project --hash
```

Hash values are stored in the `assets` table (`content_hash TEXT` column). The column is
populated on first scan with `--hash`; subsequent scans without `--hash` use mtime only.

---

### `--no-progress`

Suppress the animated progress bar. Output falls back to the static summary lines.

Progress is displayed by default when stderr is a TTY. It is automatically disabled
when stderr is redirected (pipes, CI environments, `--format github-actions`).

```bash
uasset-lens scan ./Project --no-progress
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts (auto-removes stale records) |
