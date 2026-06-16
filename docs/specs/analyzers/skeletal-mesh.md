# SkeletalMesh Analysis — Specification

## Purpose

Analyze `SkeletalMesh` assets to extract LOD count, bone count, triangle count, and morph
target presence. Enables lint rules that catch missing LODs, oversized skeletons, and
expensive morph target configurations.

---

## Scope

| Asset Class | Covered |
|---|---|
| `SkeletalMesh` | Yes |
| `Skeleton` | No (bone count derived from SkeletalMesh directly) |

---

## Metadata Extracted (Scanner Additions)

All fields are read from the `SkeletalMesh` export property stream and embedded render data.

| Field | Binary Type | Source Location | Notes |
|---|---|---|---|
| `lod_count` | `int32` | `LODInfo` array length in export properties | Number of LOD levels including LOD0 |
| `bone_count` | `int32` | `RefSkeleton.RawRefBoneInfo` array length | Number of bones in the reference skeleton |
| `triangle_count_lod0` | `int32` | `FSkeletalMeshRenderData` section sum at LOD0 | Total triangles across all sections at LOD0 |
| `has_morph_targets` | `bool` | `MorphTargets` array non-empty | Whether morph target (blend shape) data is present |

Fields absent from the binary are stored as `NULL`.

---

## Database Schema

```sql
CREATE TABLE skeletal_mesh_metadata (
    asset_path          TEXT PRIMARY KEY REFERENCES assets(path) ON DELETE CASCADE,
    lod_count           INTEGER,
    bone_count          INTEGER,
    triangle_count_lod0 INTEGER,
    has_morph_targets   INTEGER  -- 0 or 1
);
```

---

## Lint Rules

| Rule ID | Severity | Condition | Rationale |
|---|---|---|---|
| `lint/skeletal-mesh/no-lod` | Warning | `lod_count = 1` and `triangle_count_lod0 > 5000` | Character/creature meshes at full detail across all distances is a common performance mistake |
| `lint/skeletal-mesh/excess-bones` | Warning | `bone_count > 256` | Many mobile GPU hardware skinning paths are capped at 256 bones; exceeding this forces software skinning fallback |
| `lint/skeletal-mesh/high-poly-morph` | Warning | `has_morph_targets = 1` and `triangle_count_lod0 > 20000` | Morph target deltas are stored per-vertex; high triangle counts multiply memory and upload cost |

---

## Budget Rules

```toml
[budget]
SkeletalMesh = "100MB"  # per-file default; configurable in .uasset-lens.toml
```

---

## UE5 Binary Format Notes

`RefSkeleton` is a `StructProperty` sub-object embedded in the `SkeletalMesh` export.
`RawRefBoneInfo` is an `ArrayProperty` of `FMeshBoneInfo` structs; its length gives
`bone_count`. This field is serialized in editor assets and generally available even in
cooked builds.

`MorphTargets` is an `ArrayProperty` of object references. Its non-zero length indicates
morph target presence. In cooked assets, morph target delta data is stored in bulk data
sections after the property stream.

Triangle counts at LOD0 come from `FSkeletalMeshRenderData`, which is platform-specific bulk
data. If unavailable (editor-only asset without cooked render data), store `NULL` and skip
`lint/skeletal-mesh/no-lod` and `lint/skeletal-mesh/high-poly-morph` for that asset.
