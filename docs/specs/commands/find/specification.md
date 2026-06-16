# `find` Command — Specification

## Purpose

Search and filter assets in the DB by type, size, path pattern, or dependency
relationship. The Swiss-army-knife query tool for exploring the asset database.

```bash
uasset-lens find ./Project --type Texture2D --larger-than 4194304
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Always (unless execution error) |
| `2` | Execution error |

---

## Text Output

```
$ uasset-lens find ./Project --type Texture2D --larger-than 4194304

/Game/Characters/T_Player_D    (Texture2D, 8.2 MB)
/Game/Environments/T_Rock_D    (Texture2D, 6.4 MB)
/Game/Environments/T_Ground_D  (Texture2D, 5.1 MB)

3 assets found (19.7 MB total)
```

---

## Filter Examples

```bash
# By type
uasset-lens find ./Project --type Blueprint
uasset-lens find ./Project --type StaticMesh

# By size
uasset-lens find ./Project --larger-than 10485760     # > 10 MB
uasset-lens find ./Project --smaller-than 1024        # < 1 KB

# Combined size range
uasset-lens find ./Project --larger-than 4194304 --smaller-than 10485760

# Unreferenced assets only
uasset-lens find ./Project --unreferenced

# By glob path pattern
uasset-lens find ./Project --path "**/Characters/**"
uasset-lens find ./Project --path "**/Plugins/**"

# Sorted by size
uasset-lens find ./Project --type Texture2D --sort-by-size
```

---

## Dependency Relationship Filters

```bash
# Assets that reference /Game/Materials/M_Rock (direct + transitive)
uasset-lens find ./Project --refs /Game/Materials/M_Rock

# Assets that /Game/Characters/BP_Player directly depends on
uasset-lens find ./Project --deps /Game/Characters/BP_Player
```

`--refs` and `--deps` accept game paths only (not filesystem paths).

---

## JSON Output (`--format json`)

```json
[
  { "path": "/Game/Characters/T_Player_D",   "type": "Texture2D", "file_size": 8601600 },
  { "path": "/Game/Environments/T_Rock_D",   "type": "Texture2D", "file_size": 6710886 },
  { "path": "/Game/Environments/T_Ground_D", "type": "Texture2D", "file_size": 5349990 }
]
```
