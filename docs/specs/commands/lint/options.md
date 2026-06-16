# `lint` Command — Options

## Synopsis

```
uasset-lens lint <project_dir>
```

---

## Arguments

| Argument | Required | Description |
|---|---|---|
| `<project_dir>` | Yes | Path to the UE project root or Content directory |

---

## Options

### `--fix`

Dry-run mode: show which violations COULD be auto-fixed and what the fix would be.
Does not modify any files.

Only naming-convention violations are auto-fixable in v0.4.0: the tool suggests the
corrected asset name but cannot rename assets directly (UE Editor must do that).

```bash
uasset-lens lint ./Project --fix
```

Output:
```
Auto-fixable violations (2):

  [naming] /Game/Meshes/Rock.uasset
    StaticMesh missing prefix 'SM_'
    Suggested rename: Rock → SM_Rock

  [naming] /Game/UI/MainMenu.uasset
    UserWidget missing prefix 'WBP_'
    Suggested rename: MainMenu → WBP_MainMenu

Non-fixable violations (2):
  [blueprint] /Game/Characters/BP_Player.uasset: EventTick node count (8) exceeds limit (5)
  [budget] /Game/Environments/T_Terrain_D.uasset: Texture2D 32.0 MB exceeds limit 4.0 MB

2 of 4 violations are auto-fixable. Use UE Editor to rename assets.
```

---

### `--only <CATEGORIES>`

Run only the specified rule categories. Comma-separated.

Valid values: `naming`, `blueprint`, `budget`

```bash
uasset-lens lint ./Project --only naming
uasset-lens lint ./Project --only naming,blueprint
```

---

### `--skip <CATEGORIES>`

Skip the specified rule categories. Comma-separated.

```bash
uasset-lens lint ./Project --skip budget
```

---

## Global Options (apply to all commands)

| Flag | Short | Description |
|---|---|---|
| `--format <text\|json\|github-actions\|sarif>` | | Output format (default: `text`) |
| `--db <path>` | | Override the database file path |
| `--config <path>` | | Override the config file path |
| `--yes` | `-y` | Skip confirmation prompts |
