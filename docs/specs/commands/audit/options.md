# `audit` Command — Options

## Synopsis

```
uasset-lens audit <project_dir> <asset_path> [options]
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root or Content directory |
| `<asset_path>` | Yes | Game path of the asset to audit (e.g. `/Game/Characters/BP_Player`) |

---

## Options

### `--format <text|json>`

Output format. Default: `text`.

`text` prints the human-readable report with sections and aligned columns.
`json` prints a single JSON object with all sections (no truncation on lists).

```bash
uasset-lens audit ./Project /Game/Characters/BP_Player --format json
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts |
