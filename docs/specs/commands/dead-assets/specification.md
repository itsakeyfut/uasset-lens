# `dead-assets` Command — Specification

## Purpose

List assets that are not referenced by any other asset in the project. These are
candidates for deletion to reduce project size and cook times.

```bash
uasset-lens dead-assets ./Project
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | No unreferenced assets found |
| `1` | One or more unreferenced assets detected |
| `2` | Execution error |

---

## What Counts as "Dead"

An asset is considered dead if its in-degree in the dependency graph is zero — no other
asset has a hard reference to it. Root-level assets (Levels, GameModes, etc.) that are
intentionally unreferenced are also reported; use `--exclude` to filter them out.

Sub-object types (`MetaData`, `BillboardComponent`, etc.) are excluded by default because
they are always co-located with a parent asset and not independently deletable. Use
`--include-all-types` to include them.

---

## Text Output

```
$ uasset-lens dead-assets ./Project

/Game/Unused/T_OldRock          (Texture2D,   2.1 MB)
/Game/Characters/SK_OldEnemy    (SkeletalMesh, 8.4 MB)
/Game/Unused/BP_Test            (Blueprint,    0.3 MB)

Dead assets: 3 found (10.8 MB total)

  By type:
    SkeletalMesh  1  8.4 MB
    Texture2D     1  2.1 MB
    Blueprint     1  0.3 MB
```

The summary is followed by a per-type breakdown — every asset type in the dead list with its
count and wasted bytes, sorted by wasted bytes descending — so a developer can see at a glance
which types dominate. In `--format json`, the same data is emitted as a `by_type` array
(`[{ "type": "Texture2D", "count": 28, "bytes": 88473600 }, ...]`) alongside `assets`.

---

## Filtering

```bash
# Only Texture2D assets
uasset-lens dead-assets ./Project --type Texture2D

# Only assets 1 MB or larger
uasset-lens dead-assets ./Project --min-size 1048576

# Exclude assets under Content/Dev/ or Content/Plugins/
uasset-lens dead-assets ./Project --exclude Dev --exclude Plugins
```

---

## Sorting

```bash
# Sort by file size, largest first
uasset-lens dead-assets ./Project --sort-by-size
```

---

## Grouping (`--group`)

```bash
# Group by asset type
uasset-lens dead-assets ./Project --group type
```

```
Texture2D (28 assets, 84.3 MB)
  /Game/Unused/T_OldRock (2.1 MB)
  ...

Blueprint (5 assets, 1.2 MB)
  /Game/Unused/BP_Test (0.3 MB)
  ...
```

```bash
# Group by top-level directory
uasset-lens dead-assets ./Project --group dir
```

```
/Game/Unused/ (15 assets, 45.2 MB)
  ...

/Game/Characters/ (3 assets, 9.1 MB)
  ...
```

---

## Soft Reference Mode (`--include-soft-refs`)

By default, only hard import-table references contribute to in-degree. With
`--include-soft-refs`, soft object path references (`FSoftObjectPath`) also count.

```bash
uasset-lens dead-assets ./Project --include-soft-refs
```

Assets reachable only via soft references remain in the output without this flag,
even if they are clearly "in use" at runtime. This flag is the conservative choice
for projects using Asset Manager, Blueprint latent loading, or similar patterns.

Requires soft reference data; see `docs/specs/analyzers/soft-ref-cycles.md`.

---

## Revival Preview (`--revival-preview`)

Shows which existing assets could logically reference each dead asset — i.e., which
assets would gain impact if the dead asset were connected to the graph.

Intended as a planning tool before deciding to delete or reactivate assets.

```bash
uasset-lens dead-assets ./Project --revival-preview
```

Output:
```
/Game/Unused/M_LegacyRock (Material, 1.2 MB)
  Candidate references:
    /Game/Meshes/SM_Rock — already references similar material M_RockNew

1 revival candidate shown. Verify in UE Editor before reconnecting.
```

---

## JSON Output (`--format json`)

```json
[
  { "path": "/Game/Unused/T_OldRock",       "type": "Texture2D",    "file_size": 2202009 },
  { "path": "/Game/Characters/SK_OldEnemy", "type": "SkeletalMesh", "file_size": 8808038 }
]
```
