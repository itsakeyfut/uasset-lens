# `crates/bp-analyzer` — Blueprint complexity analysis

## Summary

Create the `bp-analyzer` crate that wraps `BlueprintMetrics` with complexity thresholds
and warning generation.
Complete when `analyze()` returns `Some(BlueprintMetrics)` for a Blueprint fixture and
`is_complex()` fires warnings above configured thresholds.

## Design Notes

**Public API:**

```rust
pub use scanner::BlueprintMetrics;

/// Returns None for non-Blueprint assets.
pub fn analyze(metadata: &AssetMetadata) -> Option<BlueprintMetrics> {
    metadata.blueprint_metrics.clone()
}

pub struct ComplexityThresholds {
    pub max_node_count:       u32,  // default: 200
    pub max_event_tick_count: u32,  // default: 1
    pub max_cast_count:       u32,  // default: 10
    pub max_dependency_depth: u32,  // default: 5
}

impl Default for ComplexityThresholds { ... }

pub fn is_complex(metrics: &BlueprintMetrics, thresholds: &ComplexityThresholds) -> Vec<Warning>

pub struct Warning {
    pub rule: &'static str,
    pub message: String,
}
```

`is_complex()` returns one `Warning` per threshold exceeded. Called by `lint-engine`.

## Requirements

- [ ] Create `crates/bp-analyzer` crate depending on `scanner` (for `BlueprintMetrics`, `AssetMetadata`)
- [ ] Re-export `BlueprintMetrics` from `scanner`
- [ ] Implement `analyze(metadata: &AssetMetadata) -> Option<BlueprintMetrics>`
- [ ] Define `ComplexityThresholds` struct with `Default` (values: node 200, tick 1, cast 10, depth 5)
- [ ] Define `Warning` struct
- [ ] Implement `is_complex(metrics, thresholds) -> Vec<Warning>`
- [ ] Unit test: `analyze()` returns `Some` for Blueprint, `None` for Texture2D
- [ ] Unit test: `is_complex()` returns empty Vec when all metrics below thresholds
- [ ] Unit test: `is_complex()` returns a Warning for each threshold exceeded

## Related

- Depends on: #2 (BlueprintMetrics in AssetMetadata)
- Used by: #11 (lint-engine blueprint complexity rule), #13 (blueprint command)
