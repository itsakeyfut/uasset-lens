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

This command has no command-specific options.

`--format` is accepted globally but has no effect for `watch` — output is always text
because the command produces a continuous stream of events.

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts (used during initial scan) |
