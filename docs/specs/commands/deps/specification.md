# `deps` Command — Specification

## Purpose

Show the forward dependency tree of an asset — everything the asset references,
directly or transitively.

```bash
uasset-lens deps ./Project /Game/Characters/BP_Player
```

Accepts both UE game paths (`/Game/...`) and filesystem paths.

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Always (unless execution error) |
| `2` | Execution error |

---

## Text Output

```
$ uasset-lens deps ./Project /Game/Characters/BP_Player

/Game/Characters/BP_Player (Blueprint, 0.4 MB)
├─ /Game/Characters/SK_Player (SkeletalMesh, 12.3 MB)
│   ├─ /Game/Characters/T_Player_D (Texture2D, 4.1 MB)
│   └─ /Game/Characters/T_Player_N (Texture2D, 2.0 MB)
├─ /Game/Weapons/BP_Sword (Blueprint, 0.2 MB)
│   └─ /Game/Weapons/SM_Sword (StaticMesh, 1.8 MB)
└─ /Game/Materials/M_Character (Material, 0.1 MB)

Total: 6 assets, 20.9 MB (direct: 3, transitive: 3)
```

---

## Depth Limiting (`--depth`)

```bash
# Show only direct dependencies (depth 1)
uasset-lens deps ./Project /Game/Characters/BP_Player --depth 1
```

```
/Game/Characters/BP_Player (Blueprint, 0.4 MB)
├─ /Game/Characters/SK_Player (SkeletalMesh, 12.3 MB)
├─ /Game/Weapons/BP_Sword (Blueprint, 0.2 MB)
└─ /Game/Materials/M_Character (Material, 0.1 MB)

Total: 3 assets, 12.6 MB (direct: 3)
```

---

## Summary-Only Mode (`--size-only`)

Prints only the summary line without the full tree. Useful for quick size checks.

```bash
uasset-lens deps ./Project /Game/Characters/BP_Player --size-only
```

```
BP_Player: 6 assets, 20.9 MB total
```

---

## JSON Output (`--format json`)

```json
{
  "root": "/Game/Characters/BP_Player",
  "total_assets": 6,
  "total_bytes": 21943501,
  "direct_count": 3,
  "transitive_count": 3,
  "tree": [
    {
      "path": "/Game/Characters/SK_Player",
      "type": "SkeletalMesh",
      "file_size": 12897280,
      "depth": 1,
      "children": [
        { "path": "/Game/Characters/T_Player_D", "type": "Texture2D", "file_size": 4300800, "depth": 2, "children": [] },
        { "path": "/Game/Characters/T_Player_N", "type": "Texture2D", "file_size": 2097152, "depth": 2, "children": [] }
      ]
    }
  ]
}
```
