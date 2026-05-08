# `crates/scanner` — Blueprint Export property extraction

## Summary

Extend the scanner to extract Blueprint-specific metrics from Export object property data
and store them in `AssetMetadata`.
Complete when a Blueprint fixture's `node_count`, `event_tick_count`, and `cast_count`
are correctly extracted.

## Design Notes

**Target asset types:** `Blueprint`, `AnimBlueprint`, `UserWidget` only.
For all other types, Blueprint fields remain `None`.

**Metrics to extract from parsed properties:**

| Property name (in UE serialization) | Metric |
|---|---|
| Array property counting graph nodes | `node_count` |
| Presence of `K2Node_Event` with `EventName == "ReceiveTick"` | `event_tick_count` |
| Presence of `K2Node_DynamicCast` nodes | `cast_count` |
| Depth of outer Blueprint references in ImportTable | `dependency_depth` |

> **Note**: The exact UE5 property names for Blueprint graph data require investigation
> in the UE5 source (`UEdGraph`, `UK2Node`). Use a hex editor on a known Blueprint fixture
> alongside the property parser from Issue #1 to confirm names before implementing.

**`AssetMetadata` extension:**

```rust
pub struct AssetMetadata {
    // ... existing fields ...
    pub blueprint_metrics: Option<BlueprintMetrics>,
}

pub struct BlueprintMetrics {
    pub node_count:       u32,
    pub event_tick_count: u32,
    pub cast_count:       u32,
    pub dependency_depth: u32,
}
```

`dependency_depth` can be approximated by counting the number of unique `/Game/` Blueprint
imports in the ImportTable (does not require full property parsing).

## Requirements

- [ ] Define `BlueprintMetrics` struct in `crates/scanner` (will be re-exported by `bp-analyzer`)
- [ ] Add `blueprint_metrics: Option<BlueprintMetrics>` to `AssetMetadata`
- [ ] Implement Blueprint property extraction function called from `scan_files()` for Blueprint-type assets
- [ ] Populate `event_tick_count` and `cast_count` from graph node properties
- [ ] Populate `dependency_depth` from ImportTable Blueprint reference count
- [ ] Set `blueprint_metrics = None` for non-Blueprint asset types
- [ ] Integration test: Blueprint fixture → `blueprint_metrics` is `Some` with non-zero `node_count`
- [ ] Integration test: Texture2D fixture → `blueprint_metrics` is `None`

## Related

- Depends on: #1 (property parser)
- Next: #3 — bp-analyzer crate
