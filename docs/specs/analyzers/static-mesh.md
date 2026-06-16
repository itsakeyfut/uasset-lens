# StaticMesh Analysis — Specification

## Purpose

Analyze `StaticMesh` assets to extract LOD configuration, polygon count, Nanite presence,
collision setup, and lightmap resolution. Enables lint rules that catch missing LODs,
oversized lightmaps, and missing collision on interactive props.

---

## Scope

| Asset Class | Covered |
|---|---|
| `StaticMesh` | Yes |

---

## Metadata Extracted (Scanner Additions)

All fields are read from the `StaticMesh` export property stream and embedded platform data.

| Field | Binary Type | Source Location | Notes |
|---|---|---|---|
| `lod_count` | `int32` | `StaticMeshLODInfo` array length | Number of LOD levels; LOD0 is the highest-detail level |
| `triangle_count_lod0` | `int32` | `FStaticMeshSection.NumTriangles` sum at LOD0 | Total triangle count across all sections at LOD0 |
| `has_nanite` | `bool` | `NaniteSettings.bEnabled` export property | Whether Nanite mesh representation is enabled (UE5+) |
| `has_collision` | `bool` | `BodySetup` export reference presence | False if no `BodySetup` export exists or `CollisionTraceFlag = CTF_UseNoCollision` |
| `lightmap_resolution` | `int32` | Export property `LightMapResolution` | Default lightmap UV resolution; 0 means inherit from parent actor |

Fields absent from the binary are stored as `NULL`.

---

## Database Schema

```sql
CREATE TABLE static_mesh_metadata (
    asset_path          TEXT PRIMARY KEY REFERENCES assets(path) ON DELETE CASCADE,
    lod_count           INTEGER,
    triangle_count_lod0 INTEGER,
    has_nanite          INTEGER,  -- 0 or 1
    has_collision       INTEGER,  -- 0 or 1
    lightmap_resolution INTEGER
);
```

---

## Lint Rules

| Rule ID | Severity | Condition | Rationale |
|---|---|---|---|
| `lint/static-mesh/no-lod` | Warning | `lod_count = 1` and `triangle_count_lod0 > 10000` | High-poly meshes without LODs cause GPU cost to be paid at every viewing distance |
| `lint/static-mesh/high-lightmap-res` | Warning | `lightmap_resolution > 512` | Oversized lightmaps consume excessive lightmap atlas space during lighting builds |
| `lint/static-mesh/no-collision` | Warning | `has_collision = 0` | Meshes without collision may be intentional (foliage, debris) but are flagged for review; suppress per-asset |
| `lint/static-mesh/nanite-no-lod` | Info | `has_nanite = 1` and `lod_count > 1` | Nanite handles its own internal LOD; authored LODs are unused and waste disk space |

The `lint/static-mesh/no-collision` rule is intentionally noisy. Projects with many
decoration-only meshes should suppress it globally via `.uasset-lens.toml`:

```toml
[lint]
disabled = ["lint/static-mesh/no-collision"]
```

---

## Budget Rules

```toml
[budget]
StaticMesh = "50MB"  # per-file default; configurable in .uasset-lens.toml
```

---

## UE5 Binary Format Notes

`StaticMeshLODInfo` is an `ArrayProperty` of `StructProperty` entries. Its length gives
`lod_count`. Triangle counts are embedded in the cooked bulk data (`FStaticMeshRenderData`)
which is platform-specific and not always present in editor assets — in that case fall back
to the `RenderData` property if accessible, or store `NULL`.

`NaniteSettings` is a `StructProperty` with a `bEnabled` `BoolProperty` sub-field, present
in UE 5.0+ assets. Older assets (5.0 pre-release) may omit this property; treat as
`has_nanite = 0`.

`BodySetup` is referenced as a sub-object export. Its presence and the value of
`CollisionTraceFlag` (a `ByteProperty` enum) together determine `has_collision`.
