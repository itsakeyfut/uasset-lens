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

Duplicate Assets
================
[Texture duplicate] T_Rock_D (4.1 MB × 3)
  /Game/Environments/Rocks/T_Rock_D
  /Game/Environments/Cliffs/T_Rock_D
  /Game/Levels/Forest/T_Rock_D

[Same name, SkeletalMesh] SK_Mannequin (2 copies)
  /Game/ThirdPerson/Mannequin_UE4/SK_Mannequin
  /Game/ThirdPerson/Mannequins/SK_Mannequin

[Same name, mixed: Material / MaterialInstance] M_FlareMaster (2 copies)
  /Game/Niagara/MasterMaterial/M_FlareMaster  (Material)
  /Game/Niagara/Rage/M_FlareMaster            (MaterialInstance)

3 duplicate groups found.
```

The same-name header includes the shared asset type when every copy is the same type, or
`mixed: <types>` when they differ — in which case each path line is annotated with its own type.

No duplicates:

```
Duplicate Assets
================
  (no duplicate assets found)
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

Each group has a `type` (the duplicate kind: `texture-dup` or `same-name`), a `name`, a
`shared_type` (the asset type common to all copies, or `null` when they differ), and an `assets`
array where each entry carries its own `path` and `type`.

```json
[
  {
    "type": "texture-dup",
    "name": "T_Rock_D",
    "shared_type": "Texture2D",
    "assets": [
      { "path": "/Game/Environments/Rocks/T_Rock_D", "type": "Texture2D" },
      { "path": "/Game/Environments/Cliffs/T_Rock_D", "type": "Texture2D" }
    ]
  },
  {
    "type": "same-name",
    "name": "M_FlareMaster",
    "shared_type": null,
    "assets": [
      { "path": "/Game/Niagara/MasterMaterial/M_FlareMaster", "type": "Material" },
      { "path": "/Game/Niagara/Rage/M_FlareMaster", "type": "MaterialInstance" }
    ]
  }
]
```
