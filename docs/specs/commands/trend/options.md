# `trend` Command — Options

## Synopsis

```
uasset-lens trend <project_dir> [options]
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root or Content directory |

---

## Options

### `--metric <metrics>`

Comma-separated list of metrics to display. Default: all metrics.

Valid values: `assets`, `violations`, `dead-assets`, `cycles`, `file-size-total`.

```bash
uasset-lens trend ./Project --metric violations,dead-assets
uasset-lens trend ./Project --metric assets,file-size-total
```

Unknown metric names produce an error: `error: unknown metric 'foo'`.

---

### `--limit <N>`

Number of history entries to include in the trend window. Default: `20`.

Entries are selected newest-first; the oldest entry within the window forms the
baseline for the trend delta summary line.

```bash
uasset-lens trend ./Project --limit 7
```

---

### `--format <text|json>`

Output format. Default: `text`.

`json` emits a structured object with an `entries` array and a `trend` delta object.

```bash
uasset-lens trend ./Project --format json
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts |
