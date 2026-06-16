# Material Analysis (Extended) — Specification

## Purpose

Extend the existing `uasset-lens-material-analyzer` (texture sample count, instance chain
depth) with shading model, blend mode, and rendering flag metadata. Enables additional lint
rules that catch expensive GPU rendering configurations.

---

## Scope

| Asset Class | Notes |
|---|---|
| `Material` | Full metadata extraction |
| `MaterialInstance` | Chain depth tracked; inherits parent metadata at analysis time |
| `MaterialInstanceConstant` | Same as `MaterialInstance` |
| `MaterialFunction` | Dependency tracking only; no standalone metadata |

---

## Existing Analyzer Capabilities

The `uasset-lens-material-analyzer` crate already extracts:

- **Texture sample count** — number of `TextureSample` / `TextureSampleParameter` nodes in the material graph
- **Material instance chain depth** — how many `MaterialInstance` parents exist before reaching the base `Material`

These are already stored in the database. The extension defined here adds new fields to a
unified `material_metadata` table.

---

## Additional Metadata (Phase v0.3.0 Extension)

All fields are read from the `Material` export property stream.

| Field | Binary Type | Source Location | Notes |
|---|---|---|---|
| `shading_model` | `uint8` enum | Export property `ShadingModel` | e.g. `MSM_DefaultLit`, `MSM_Unlit`, `MSM_Subsurface` |
| `blend_mode` | `uint8` enum | Export property `BlendMode` | e.g. `BLEND_Opaque`, `BLEND_Masked`, `BLEND_Translucent` |
| `two_sided` | `bool` | Export property `TwoSided` | Disables backface culling; doubles rasterization cost |
| `uses_world_position_offset` | `bool` | Export property `bUsedWithSkeletalMesh` — derived | Detected via node graph presence of `WorldPositionOffset` output connection |
| `uses_distance_field_gi` | `bool` | Export property `bUsedWithDistanceFieldLighting` | Enables distance field global illumination contribution |

---

## Database Schema

The existing material analysis data is merged into a single unified table:

```sql
CREATE TABLE material_metadata (
    asset_path           TEXT PRIMARY KEY REFERENCES assets(path) ON DELETE CASCADE,
    shading_model        TEXT,
    blend_mode           TEXT,
    two_sided            INTEGER,  -- 0 or 1
    uses_wpo             INTEGER,  -- 0 or 1; uses_world_position_offset
    uses_distance_field  INTEGER,  -- 0 or 1
    texture_sample_count INTEGER,  -- from existing analyzer
    instance_chain_depth INTEGER   -- from existing analyzer
);
```

When migrating from the previous separate storage, existing texture sample count and chain
depth values are carried forward into this table.

---

## Lint Rules

### Existing Rules (Already Implemented)

| Rule ID | Severity | Description |
|---|---|---|
| `lint/material/too-many-texture-samples` | Warning | `texture_sample_count` exceeds configured threshold |
| `lint/material/deep-instance-chain` | Warning | `instance_chain_depth` exceeds configured threshold |

### New Rules (Phase v0.3.0)

| Rule ID | Severity | Condition | Rationale |
|---|---|---|---|
| `lint/material/wpo-on-opaque` | Warning | `uses_wpo = 1` and `blend_mode = BLEND_Opaque` | WPO on opaque materials prevents GPU depth pre-pass optimization; usually unintentional |
| `lint/material/translucent-two-sided` | Warning | `blend_mode = BLEND_Translucent` and `two_sided = 1` | Translucent two-sided materials render each pixel twice with OIT overhead; expensive GPU combination |
| `lint/material/subsurface-high-sample-count` | Error | `shading_model = MSM_Subsurface` and `texture_sample_count > 16` | Subsurface scattering already incurs multi-pass cost; excessive texture samples compound this significantly |

---

## Budget Rules

Budget enforcement uses the existing `[budget]` section in `.uasset-lens.toml`.

```toml
[budget]
Material = "2MB"
MaterialInstanceConstant = "512KB"
```

---

## UE5 Binary Format Notes

`ShadingModel` and `BlendMode` are serialized as `ByteProperty` (8-bit enum index) in the
tagged property stream. `TwoSided` is a `BoolProperty`. These appear in the `Material` class
export, not in `MaterialInstance` exports (which only store overrides).

`WorldPositionOffset` usage is inferred from the material's feature flag
`bUsedWithWorldPositionOffset` (`BoolProperty`), available in UE 5.1+. In earlier cooked
assets this flag may be absent; treat as `NULL`.
