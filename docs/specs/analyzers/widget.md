# Widget Blueprint Analysis — Specification

## Purpose

Analyze `WidgetBlueprint` assets to detect deeply nested widget hierarchies and excessive
UMG animation counts. Widgets are compiled Blueprints; the existing Blueprint analyzer
(`uasset-lens-bp-analyzer`) covers node counts and variable counts. This spec covers
widget-specific structure metrics.

---

## Scope

| Asset Class | Covered |
|---|---|
| `WidgetBlueprint` | Yes |

Other Blueprint types (`Blueprint`, `AnimBlueprint`, etc.) are handled by the existing
Blueprint analyzer and are out of scope here.

---

## Metadata Extracted (Scanner Additions)

Widget-specific metadata derived from the `UWidgetTree` and `UWidgetAnimation` exports
embedded within the `WidgetBlueprint` asset.

| Field | Binary Type | Source Location | Notes |
|---|---|---|---|
| `widget_depth` | `int32` | Computed from `UWidgetTree` root slot chain | Maximum nesting depth of the widget tree (root = depth 0) |
| `animation_count` | `int32` | `Animations` array length in `UWidgetBlueprint` export | Number of `UWidgetAnimation` objects defined in the widget |

**MVP fallback:** Exact `widget_depth` extraction from `UWidgetTree` is a stretch goal.
For MVP, use the asset's hard dependency count as a proxy for complexity. Store `NULL`
for `widget_depth` when exact extraction is not implemented; lint rules that depend on it
are skipped for that asset.

---

## Database Schema

```sql
CREATE TABLE widget_metadata (
    asset_path      TEXT PRIMARY KEY REFERENCES assets(path) ON DELETE CASCADE,
    widget_depth    INTEGER,  -- NULL until exact extraction is implemented
    animation_count INTEGER
);
```

---

## Lint Rules

| Rule ID | Severity | Condition | Rationale |
|---|---|---|---|
| `lint/widget/deep-hierarchy` | Warning | `widget_depth > 8` | Deep widget trees increase Slate's layout invalidation cost; each extra level multiplies the affected subtree on resize or tick |
| `lint/widget/many-animations` | Warning | `animation_count > 20` | Large animation counts cause `UMG` pre-tick overhead and make widget assets difficult to maintain |

Rules that depend on `widget_depth` are skipped silently when the value is `NULL`
(MVP fallback active).

---

## Budget Rules

```toml
[budget]
WidgetBlueprint = "2MB"  # per-file default; configurable in .uasset-lens.toml
```

---

## UE5 Binary Format Notes

A `WidgetBlueprint` asset contains multiple exports. The exports of interest are:

- `UWidgetBlueprint` — the root export; contains the `Animations` `ArrayProperty` of
  `ObjectProperty` references pointing to `UWidgetAnimation` sub-objects.
- `UWidgetTree` — a sub-object export containing the slot hierarchy. Each slot has a
  `Content` `ObjectProperty` pointing to a child widget export.

Widget tree depth is computed by traversing the `Content` chain from the root slot until
no further child is found. This traversal must be bounded (max depth 64) to guard against
malformed assets.

`UWidgetAnimation` exports are identified by class name. Their count gives
`animation_count` directly.
