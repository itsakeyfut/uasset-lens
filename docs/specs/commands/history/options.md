# `history` Command — Options

## Synopsis

```
uasset-lens history <project_dir> [options]
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root or Content directory |

---

## Options

### `--limit <N>`

Maximum number of history entries to display. Default: `20`.

Entries are selected newest-first; older entries beyond the limit are not shown.

```bash
uasset-lens history ./Project --limit 10
uasset-lens history ./Project --limit 5
```

---

### `--format <text|json>`

Output format. Default: `text`.

`text` prints the tabular report. `json` prints a JSON object with an `entries` array.

```bash
uasset-lens history ./Project --format json
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts |
