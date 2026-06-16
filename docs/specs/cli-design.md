# CLI Detailed Design

## DB File Location

Auto-generated at `<project_dir>/.uasset-lens/uasset-lens.db`.

```
/MyProject/
  ├─ Content/           # UE content
  ├─ .uasset-lens/
  │   └─ uasset-lens.db  # ← auto-created on scan (add to .gitignore)
  └─ .uasset-lens.toml  # config file (optional)
```

Override with `--db <path>` for CI use cases.

## Content Root Resolution

Interpreting `<project_dir>`:

1. If `<project_dir>/Content/` exists → `content_root = <project_dir>/Content/`
2. Otherwise → `content_root = <project_dir>` (Content directory passed directly)

For commands like `impact` that receive only an asset path, the tool walks up the directory
tree to auto-locate `.uasset-lens/uasset-lens.db`.

## Behavior When scan Has Not Been Run

If the DB is missing or empty when any other command runs, exit with an error:

```
Error: no scan data found.
Run 'uasset-lens scan <project_dir>' first.
```

## Exit codes

Three values, Clippy-style. Usable as CI quality gates.

| Code | Meaning |
|------|---------|
| `0` | Success — no issues found |
| `1` | Issues detected (dead asset, circular dependency, impact found, etc.) |
| `2` | Execution error (I/O error, DB not created, parse failure, etc.) |

```bash
# CI usage examples
uasset-lens graph --cycles-only ./Project || exit 1   # fail build on circular deps
uasset-lens dead-assets ./Project                     # exit 1 on detection (warning)
```

## Common Flags

| Flag | Description |
|------|-------------|
| `--format <text\|json>` | Output format (default: `text`) |
| `--db <path>` | Override the DB path |
| `-y` / `--yes` | Skip confirmation prompts (for CI) |

## Command Reference

### `scan <project_dir>`

Scans all `.uasset` / `.umap` files under Content and updates the DB.

**Important**: The scan command never modifies `.uasset` files themselves. It only operates on DB records.

```
Options:
  --full-scan    Force re-scan of all files regardless of mtime
  -y / --yes     Skip the DB cleanup confirmation prompt (for CI)

Output (text):
  Scanning ./MyProject/Content... (1000 files)
    + 3 new assets indexed
    ~ 5 assets updated (mtime changed)
    ? 2 assets removed from disk

  The following DB records have no corresponding file on disk:
    /Game/Old/BP_Deprecated.uasset
    /Game/Temp/M_Test.uasset
  Remove these records from DB? [y/N]: y

  ✓ 998 assets total, 2 records cleaned, 2 skipped (parse error)

  Skipped:
    WARN Content/Broken/BP_X.uasset: invalid magic number
    WARN Content/Old/M_Y.uasset: unsupported version
```

With `-y`, auto-delete without prompting.

---

### `graph <project_dir>`

Displays a dependency graph summary and circular dependencies.

```
Options:
  --cycles-only    Show circular dependencies only

Output (text):
  Dependency Graph Summary
    Total assets   : 998
    Total edges    : 4,231
    Circular deps  : 2 cycles detected

  Cycles:
    [1] BP_Player → BP_Enemy → BP_GameMode → BP_Player
    [2] M_Rock → MF_Shared → M_Rock
```

---

### `dead-assets <project_dir>`

Lists assets not referenced by any other asset.

```
Options:
  --type <AssetType>    Filter by type

Output (text):
  /Game/Unused/T_OldTexture          (Texture2D, 2.1 MB)
  /Game/Characters/SK_OldEnemy       (SkeletalMesh, 8.4 MB)
  ...

  Dead Assets (47 found)
```

---

### `impact <asset_path>`

Lists assets that would break if the given asset were deleted or renamed.

`<asset_path>` accepts both a game path (`/Game/...`) and a filesystem path.

```
Output (text):
  Impact Analysis: /Game/Characters/BP_Player

  Direct referencing (3):
    /Game/Levels/L_Main.umap
    /Game/UI/WBP_HUD.uasset
    /Game/GameModes/BP_GameMode.uasset

  Transitive referencing (12):
    /Game/Levels/L_Tutorial.umap
    ... (9 more)

  Total impact: 12 assets
```

---

### `redirectors <project_dir>`

Detects and lists Redirector assets in the project.

**Phase 1 scope**: only detects and lists assets of type `ObjectRedirector`.
Redirect target resolution (detecting broken redirectors) is Phase 2+.

```
Output (text):
  Redirectors (5 found)
    /Game/Characters/OldName.uasset
    /Game/Meshes/SM_OldRock.uasset
    /Game/Materials/M_Deprecated.uasset
    /Game/UI/WBP_OldWidget.uasset
    /Game/Blueprints/BP_OldEnemy.uasset

  Note: redirect target resolution is available in Phase 2 analysis.
```

---

### `find <project_dir> [options]`

Searches and filters assets using the DB.

```
Options:
  --type <AssetType>      Filter by type (e.g. Texture2D, Blueprint)
  --larger-than <bytes>   File size lower bound
  --smaller-than <bytes>  File size upper bound
  --unreferenced          Show only unreferenced assets
  --path <pattern>        Match by path pattern (glob)

Examples:
  uasset-lens find ./Project --type Texture2D --larger-than 4096
  uasset-lens find ./Project --unreferenced --type StaticMesh
  uasset-lens find ./Project --path "**/Characters/**"
```

---

## JSON Output Format (`--format json`)

With `--format json`, each command writes a single JSON value (object or array) to stdout.
On error, exit code `2` and write the error message to stderr (no JSON error envelope).

### `scan`

```json
{
  "assets_total": 998,
  "new":          3,
  "updated":      5,
  "removed":      2,
  "skipped": [
    { "path": "Content/Broken/BP_X.uasset", "reason": "invalid magic number" }
  ]
}
```

### `graph`

```json
{
  "total_assets": 998,
  "total_edges":  4231,
  "cycles": [
    ["/Game/BP_Player", "/Game/BP_Enemy", "/Game/BP_GameMode", "/Game/BP_Player"],
    ["/Game/M_Rock", "/Game/MF_Shared", "/Game/M_Rock"]
  ]
}
```

### `dead-assets`

```json
[
  { "path": "/Game/Unused/T_OldTexture", "type": "Texture2D",    "file_size": 2097152 },
  { "path": "/Game/Characters/SK_OldEnemy", "type": "SkeletalMesh", "file_size": 8808038 }
]
```

### `impact`

```json
{
  "target":     "/Game/Characters/BP_Player",
  "direct":     ["/Game/Levels/L_Main", "/Game/UI/WBP_HUD", "/Game/GameModes/BP_GameMode"],
  "transitive": ["/Game/Levels/L_Tutorial"],
  "total":      4
}
```

### `redirectors`

```json
[
  { "path": "/Game/Characters/OldName",       "type": "ObjectRedirector" },
  { "path": "/Game/Meshes/SM_OldRock",        "type": "ObjectRedirector" }
]
```

### `find`

```json
[
  { "path": "/Game/Textures/T_Rock_D", "type": "Texture2D", "file_size": 4194304 },
  { "path": "/Game/Textures/T_Rock_N", "type": "Texture2D", "file_size": 2097152 }
]
```

---

## `.uasset-lens.toml` Config File (Phase 1 minimum spec)

Placed in the project root and checked into git for team sharing. If absent, defaults apply.

### Phase 1 supported fields

```toml
# .uasset-lens.toml

[scan]
# Paths to exclude from scanning (relative to content_root, prefix match)
exclude_paths = [
    "Content/Dev/",
    "Content/Test/",
    "Content/Developers/",
]
```

Naming conventions, size budgets, and other fields will be added in Phase 3+.
