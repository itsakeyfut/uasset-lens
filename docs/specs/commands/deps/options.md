# `deps` Command — Options

## Synopsis

```
uasset-lens deps <project_dir> <asset_path> [options]
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root or Content directory |
| `<asset_path>` | Yes | Asset to inspect — UE game path (`/Game/...`) or filesystem path |

---

## Options

### `--depth <N>`

Maximum recursion depth for the dependency tree (default: unlimited).

`--depth 1` shows only direct dependencies. `--depth 2` shows dependencies of
dependencies, and so on.

```bash
uasset-lens deps ./Project /Game/Characters/BP_Player --depth 1
uasset-lens deps ./Project /Game/Characters/BP_Player --depth 3
```

---

### `--size-only`

Print only the summary line (total asset count and size), not the full tree.

```bash
uasset-lens deps ./Project /Game/Characters/BP_Player --size-only
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts |
