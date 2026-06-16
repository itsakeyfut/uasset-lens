# `blueprint` Command — Options

## Synopsis

```
uasset-lens blueprint <project_dir>
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root or Content directory |

---

## Options

This command has no command-specific options. The ranking always covers all Blueprint
family assets (`Blueprint`, `AnimBlueprint`, `UserWidget`, `ControlRigBlueprint`, etc.).

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts |
