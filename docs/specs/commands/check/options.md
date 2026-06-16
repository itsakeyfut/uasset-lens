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

```bash
uasset-lens check ./Project --verbose
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts |
