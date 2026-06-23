# Texture Analysis — Specification

## Purpose

Analyze `Texture2D`, `TextureCube`, `Texture2DArray`, and `VolumeTexture` assets to extract
compression settings, mip generation configuration, and source dimensions. Enables lint rules
that catch common texture authoring mistakes and enforces per-type file size budgets.

---

## Scope

| Asset Class | Covered |
|---|---|
| `Texture2D` | Yes |
| `TextureCube` | Yes |
| `Texture2DArray` | Yes |
| `VolumeTexture` | Yes |

---

## Metadata Extracted (Scanner Additions)

The scanner currently extracts only `class_name` and dependencies. Texture analysis adds the
following fields from the UE5 export property stream.

| Field | Binary Type | Source Location | Notes |
|---|---|---|---|
| `texture_compression_settings` | `uint32` enum | Export property `CompressionSettings` | e.g. `TC_Default`, `TC_NormalMap`, `TC_BC7` |
| `mip_gen_settings` | `uint32` enum | Export property `MipGenSettings` | e.g. `TMGS_FromTextureGroup`, `TMGS_NoMipmaps` |
| `has_alpha_channel` | `bool` | `FTexturePlatformData` in export | Whether the source has a non-trivial alpha |
| `texture_group` | `uint32` enum | Export property `LODGroup` | e.g. `TEXTUREGROUP_World`, `TEXTUREGROUP_Character` |
| `source_size_x` | `int32` | `FTexturePlatformData.SizeX` | Original source width in pixels |
| `source_size_y` | `int32` | `FTexturePlatformData.SizeY` | Original source height in pixels |

All fields are read from the cooked export property table. Fields absent from the binary
(e.g. default values not serialized) are stored as `NULL`.

---

## Database Schema

The `assets` table is keyed by an integer `id` (with a `UNIQUE asset_path`), so the foreign key
references `assets(id)`, matching the existing `blueprint_metrics` table.

```sql
CREATE TABLE texture_metadata (
    asset_id      INTEGER PRIMARY KEY REFERENCES assets(id) ON DELETE CASCADE,
    compression   TEXT,
    mip_gen       TEXT,
    has_alpha     INTEGER,  -- 0 or 1; NULL if unknown
    texture_group TEXT,
    source_x      INTEGER,
    source_y      INTEGER
);
```

`compression`, `mip_gen`, and `texture_group` store the enum **value name** as text
(e.g. `TC_Default`, `TEXTUREGROUP_World`) — in cooked UE5 assets these are serialized as
`ByteProperty` enum values (FName), not raw `uint32` indices.

**MVP scope (#299):** the scanner currently populates `compression`, `mip_gen`, and
`texture_group` from the tagged-property stream. `has_alpha`, `source_x`, and `source_y` require
parsing `FTexturePlatformData` (and `has_alpha` has no field in its documented layout); these
columns exist but are written as `NULL` until a follow-up issue adds that extraction.

---

## Lint Rules

| Rule ID | Severity | Condition | Rationale |
|---|---|---|---|
| `lint/texture/non-power-of-two` | Warning | `source_x` or `source_y` is not a power-of-two | Non-POT textures cannot generate complete mip chains and may cause rendering artifacts |
| `lint/texture/missing-mip-maps` | Warning | `mip_gen = TMGS_NoMipmaps` and `source_x > 64` and `source_y > 64` | Large textures without mipmaps cause aliasing and GPU cache thrashing at distance |
| `lint/texture/uncompressed` | Error | `compression = TC_None` and `source_x > 512` and `source_y > 512` | Uncompressed textures above 512×512 consume excessive GPU memory |
| `lint/texture/has-alpha-but-opaque` | Warning | `has_alpha = 1` and `texture_group = TEXTUREGROUP_World` | World textures rarely need transparency; an alpha channel doubles memory usage on some formats |

Lint rule thresholds are evaluated against extracted metadata at query time. All rules can
be suppressed per-asset via a `.uasset-lens-suppress` comment in a sidecar `.ini` file
(future; not part of MVP).

---

## Budget Rules

Budget enforcement uses the existing `[budget]` section in `.uasset-lens.toml`.

```toml
[budget]
Texture2D = "8MB"     # per-file limit; default if omitted
TextureCube = "16MB"
```

**Phase 2 extension (not MVP):** A `source_resolution` budget will allow declaring a maximum
source dimension, e.g. `max_source_dimension = 4096` for non-hero textures. This requires
storing the `source_x` / `source_y` fields defined above.

---

## UE5 Binary Format Notes

`FTexturePlatformData` is embedded in the export data after the standard property stream
(after the `None` tag that terminates serialized properties). The layout is:

```
SizeX        int32
SizeY        int32
PackedData   uint32   -- encodes NumSlices and OptData flags
PixelFormat  FString
```

`CompressionSettings` and `LODGroup` are serialized as tagged properties before the `None`
terminator and are read as `UInt32Property` (enum index).
