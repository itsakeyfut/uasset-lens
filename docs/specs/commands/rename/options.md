# `rename` Command — Options

## Synopsis

```
uasset-lens rename <project_dir> <source_path> <destination_path> [options]
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root or Content directory |
| `<source_path>` | Yes | Current game path of the asset (e.g. `/Game/Characters/BP_OldName`) |
| `<destination_path>` | Yes | Desired new game path (e.g. `/Game/Characters/BP_NewName`) |

---

## Options

### `--format <text|json>`

Output format. Default: `text`.

`text` prints the human-readable simulation report.
`json` prints a structured JSON object.

```bash
uasset-lens rename ./Project /Game/Old/BP_Foo /Game/New/BP_Foo --format json
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts |

---

## Notes

This command is always a dry-run. There is no `--apply` flag. Actual renaming must be
performed using UE Editor's built-in asset rename and "Fix Up Redirectors" workflow.
