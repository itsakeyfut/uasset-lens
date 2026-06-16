# `stats` Command — Specification

## Purpose

Show a size and composition overview of the project — total assets, breakdown by type
and folder, largest assets, and graph statistics.

```bash
uasset-lens stats ./Project
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
$ uasset-lens stats ./Project

Project Overview: ./Project
  Total assets : 1,024
  Total size   : 2.1 GB

By Type (top 10):
  Texture2D      412 assets   1.2 GB  (57.1%)
  StaticMesh     183 assets 412.3 MB  (19.1%)
  Blueprint      154 assets  38.5 MB   (1.8%)
  Material        98 assets  19.6 MB   (0.9%)
  SoundWave       67 assets 201.0 MB   (9.3%)
  SkeletalMesh    42 assets 189.0 MB   (8.7%)
  ...

By Folder (top 5):
  /Game/Characters/   218 assets 412.8 MB
  /Game/Environments/ 196 assets 687.2 MB
  /Game/UI/            98 assets  24.5 MB
  /Game/Weapons/       76 assets 156.3 MB
  /Game/GameModes/     43 assets   8.6 MB

Largest Assets (top 10):
  /Game/Environments/T_Terrain_D   (Texture2D,   32.0 MB)
  /Game/Characters/SK_BossEnemy    (SkeletalMesh, 28.4 MB)
  /Game/Audio/SFX_Ambience_01      (SoundWave,    18.2 MB)
  ...

Graph Statistics:
  Total edges       : 4,231
  Avg out-degree    : 4.1
  Unreferenced      : 47 assets
  Circular deps     : 2 cycles
```

---

## Controlling Output Volume (`--top`)

```bash
# Show top 20 types, top 10 folders, top 20 largest assets
uasset-lens stats ./Project --top 20

# Show all (no truncation)
uasset-lens stats ./Project --top 0
```

Default: 10 types, 5 folders, 10 largest assets.

---

## JSON Output (`--format json`)

```json
{
  "total_assets": 1024,
  "total_bytes": 2254857830,
  "by_type": [
    { "type": "Texture2D",   "count": 412, "bytes": 1288490188 },
    { "type": "StaticMesh",  "count": 183, "bytes": 432340377  }
  ],
  "by_folder": [
    { "folder": "/Game/Characters/",   "count": 218, "bytes": 432820224 },
    { "folder": "/Game/Environments/", "count": 196, "bytes": 720510566 }
  ],
  "largest_assets": [
    { "path": "/Game/Environments/T_Terrain_D", "type": "Texture2D", "file_size": 33554432 }
  ],
  "graph": {
    "total_edges": 4231,
    "avg_out_degree": 4.1,
    "unreferenced_count": 47,
    "cycle_count": 2
  }
}
```
