# `stats` Command — Options

## Synopsis

```
uasset-lens stats <project_dir> [options]
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root or Content directory |

---

## Options

### `--top <N>`

Number of entries to show in the "By Type", "By Folder", and "Largest Assets" sections.

- Default: 10 types, 5 folders, 10 assets
- `0`: show all entries without truncation

```bash
uasset-lens stats ./Project --top 20
uasset-lens stats ./Project --top 0   # show all
```

---

### `--diff <NAME>`

Compare current stats against a named baseline and show what changed.

Requires a baseline saved via `uasset-lens baseline save ./Project <NAME>`.

Shows delta (+/-) for: total assets, total size, violations, dead assets, cycles.

```bash
uasset-lens stats ./Project --diff main
uasset-lens stats ./Project --diff before-refactor
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts |
