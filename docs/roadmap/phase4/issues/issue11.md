# `crates/material-analyzer` — Material complexity metrics

## Summary

Create the `material-analyzer` crate that computes texture sample count and
`MaterialInstance` chain depth for material assets.
Complete when `analyze()` returns `MaterialMetrics` with correct values for a
Material fixture.

## Design Notes

**`MaterialMetrics`:**

```rust
pub struct MaterialMetrics {
    pub texture_sample_count: u32,
    pub instance_chain_depth: u32,
}
```

**Texture sample count:**
Read from Blueprint-style property parsing (Issue #1): look for `ArrayProperty` named
something like `Expressions` and count entries of type `MaterialExpressionTextureSample`.

> **Note**: The exact property name requires investigation on a Material fixture.
> Use the property parser from Phase 4 Issue #1 on a known Material asset.

**Instance chain depth:**
Walk the `DependencyGraph` from the `MaterialInstance` node, following dependency edges
through other `MaterialInstance` nodes until reaching a `Material` node.
Count the number of hops.

```rust
pub fn analyze(
    metadata: &AssetMetadata,
    graph: &DependencyGraph,
) -> Option<MaterialMetrics>
```

Returns `None` for non-Material and non-MaterialInstance assets.

## Requirements

- [ ] Create `crates/material-analyzer` crate
- [ ] Define `MaterialMetrics` struct
- [ ] Implement `analyze(metadata, graph) -> Option<MaterialMetrics>`
- [ ] Populate `texture_sample_count` from Export property parsing (Material only)
- [ ] Populate `instance_chain_depth` by walking the dependency graph (MaterialInstance only)
- [ ] Return `None` for non-Material/non-MaterialInstance assets
- [ ] Unit test: Material fixture → `Some(MaterialMetrics)` with non-zero texture count
- [ ] Unit test: chain of 3 MaterialInstances → `instance_chain_depth == 3`
- [ ] Unit test: StaticMesh → `None`

## Related

- Depends on: #1 (property parser), Phase 2 Issue #1 (DependencyGraph)
- Used by: #14 (lint command can add material complexity rules in future)
