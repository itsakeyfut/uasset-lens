# `baseline` Command — Options

## Synopsis

```
uasset-lens baseline <subcommand> <project_dir> [NAME] [options]
```

---

## Subcommands

| Subcommand | Description |
|---|---|
| `list` | List all saved baselines |
| `save <NAME>` | Run checks and save result as a named baseline |
| `delete <NAME>` | Delete the named baseline |
| `diff <NAME>` | Compare current violations against the named baseline |

---

## Arguments

| Argument | Required by | Description |
|---|---|---|
| `<project_dir>` | All | Path to the UE project root or Content directory |
| `<NAME>` | `save`, `delete`, `diff` | Baseline name (alphanumeric, hyphens, underscores) |

---

## Options

### `--format <text|json>`

Output format. Default: `text`.

Applies to `list` and `diff`. `save` and `delete` always use text output.

```bash
uasset-lens baseline diff ./Project main --format json
uasset-lens baseline list ./Project --format json
```

---

### `--yes` / `-y`

Skip confirmation prompts.

Applies to `save` (overwrite existing baseline) and `delete`.

```bash
uasset-lens baseline save ./Project main --yes
uasset-lens baseline delete ./Project before-refactor --yes
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts |
