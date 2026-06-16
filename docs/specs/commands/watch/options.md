# `watch` Command — Options

## Synopsis

```
uasset-lens watch <project_dir>
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root or Content directory |

---

## Options

### `--no-cycle-check`

Disable dependency cycle detection after each file change.

By default, `watch` re-checks for new cycles after every file change. This can be
slow on large projects (>50k assets). Use this flag to trade cycle detection for
lower per-event latency.

```bash
uasset-lens watch ./Project --no-cycle-check
```

---

`--format` is accepted globally but has no effect for `watch` — output is always text
because the command produces a continuous stream of events.

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts (used during initial scan) |
