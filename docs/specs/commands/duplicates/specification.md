# `duplicates` Command — Specification

## Purpose

List asset groups that appear to be duplicates. Detects two categories:
- **Texture duplicates**: same name, same file size, same type
- **Same-name assets**: assets with identical names at different paths (excluding groups
  already identified as texture duplicates)

```bash
uasset-lens duplicates ./Project
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | No duplicate groups found |
| `1` | One or more duplicate groups detected |
| `2` | Execution error |

---

## Text Output

```
$ uasset-lens duplicates ./Project

[texture-dup] T_Rock_D (Texture2D, 4.1 MB × 3)
  /Game/Environments/Rocks/T_Rock_D
  /Game/Environments/Cliffs/T_Rock_D
  /Game/Levels/Forest/T_Rock_D

[same-name] BP_Enemy (Blueprint)
  /Game/Characters/Enemies/BP_Enemy
  /Game/Characters/Bosses/BP_Enemy

Duplicate groups: 2 (5 assets)
```

No duplicates:

```
Duplicates: none found
```

---

## Detection Rules

### `texture-dup`

Assets where ALL of the following match:
- Same asset name (last path segment)
- Same `AssetType` (must be a Texture family type)
- Same file size (in bytes)

### `same-name`

Assets where the last path segment is identical, regardless of type or location.
Groups already identified as `texture-dup` are excluded to avoid double-reporting.

---

## JSON Output (`--format json`)

```json
[
  {
    "kind": "texture-dup",
    "name": "T_Rock_D",
    "type": "Texture2D",
    "file_size": 4300800,
    "paths": [
      "/Game/Environments/Rocks/T_Rock_D",
      "/Game/Environments/Cliffs/T_Rock_D",
      "/Game/Levels/Forest/T_Rock_D"
    ]
  },
  {
    "kind": "same-name",
    "name": "BP_Enemy",
    "type": null,
    "file_size": null,
    "paths": [
      "/Game/Characters/Enemies/BP_Enemy",
      "/Game/Characters/Bosses/BP_Enemy"
    ]
  }
]
```
