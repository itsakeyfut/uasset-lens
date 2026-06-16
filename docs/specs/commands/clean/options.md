# `clean` Command — Options

## Synopsis

```
uasset-lens clean <project_dir> [options]
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root or Content directory |

---

## Options

### `--dry-run`

List deletion targets and exit without deleting any files.

```bash
uasset-lens clean ./Project --dry-run
```

---

### `--min-size <BYTES>`

Exclude assets smaller than this many bytes from the deletion set.

```bash
# Delete only dead assets 1 MB or larger
uasset-lens clean ./Project --min-size 1048576
```

---

### `--exclude <PATTERN>`

Exclude assets whose path contains the given substring from the deletion set. Repeatable.

```bash
uasset-lens clean ./Project --exclude Dev
uasset-lens clean ./Project --exclude Dev --exclude Plugins
```

---

### `--path <PATTERN>`

Filter by glob path pattern. Only dead assets matching this pattern are deleted.

```bash
uasset-lens clean ./Project --path "**/Unused/**"
uasset-lens clean ./Project --path "**/Characters/**"
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompt — deletes without asking |
