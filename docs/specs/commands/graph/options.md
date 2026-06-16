# `graph` Command — Options

## Synopsis

```
uasset-lens graph <project_dir> [options]
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root or Content directory |

---

## Options

### `--cycles-only`

Print only circular dependencies; suppress the graph summary header.

Exits `1` if any cycles are found — useful as a CI gate.

```bash
uasset-lens graph ./Project --cycles-only
```

---

### `--full-cycles`

Show all nodes in long cycles instead of collapsing intermediate nodes.

By default, cycles with many nodes are shown as `A → B → ... (N more) → A`.
With `--full-cycles`, every node in the cycle is printed.

```bash
uasset-lens graph ./Project --full-cycles
uasset-lens graph ./Project --cycles-only --full-cycles
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts |
