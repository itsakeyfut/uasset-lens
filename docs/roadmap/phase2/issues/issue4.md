# `crates/dead-asset-detector` — detect unreferenced assets

## Summary

Create the `dead-asset-detector` crate and implement `detect()`, which identifies
assets with no incoming references in the dependency graph.
Complete when `detect()` correctly returns unreferenced assets from a test graph.

## Design Notes

An asset is "dead" (unreferenced) when `graph.in_degree(path) == 0`.

```rust
pub fn detect(graph: &DependencyGraph) -> Vec<AssetPath>
```

Iterate over `graph.nodes()`, collect paths where `in_degree == 0`.

This crate is a pure function layer — no DB access, no IO.
It depends only on `dependency-graph` (and transitively `shared`).

**Edge cases to test:**

| Graph | Expected |
|---|---|
| All nodes referenced | empty Vec |
| All nodes isolated (no edges) | all nodes returned |
| Mix of referenced and unreferenced | only unreferenced returned |

> **Note**: The `detect()` result will include assets that are genuinely unused AND
> assets that are "root" assets (e.g., the top-level map file). The CLI command can
> let the user filter by `--type` to narrow down noise.

## Requirements

- [ ] Create `crates/dead-asset-detector` crate depending on `dependency-graph`
- [ ] Implement `detect(graph: &DependencyGraph) -> Vec<AssetPath>`
- [ ] Collect all nodes where `graph.in_degree(path) == 0`
- [ ] Unit test: graph where all nodes are referenced → empty result
- [ ] Unit test: graph where all nodes are isolated → all nodes returned
- [ ] Unit test: mixed graph → only the unreferenced nodes returned

## Related

- Depends on: Phase 2 Issue #1 (DependencyGraph + in_degree)
- Used by: Phase 2 Issue #7 (dead-assets command)
