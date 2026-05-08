# `crates/dependency-graph` — `find_impact()`

## Summary

Implement `find_impact()` on `DependencyGraph` to return the direct and transitive
set of assets that would be affected if the target asset were deleted.
Complete when direct and transitive referencing assets are correctly separated in
`ImpactResult`.

## Design Notes

**`ImpactResult` type:**

```rust
pub struct ImpactResult {
    pub direct:     Vec<AssetPath>,   // assets that directly reference target (1 hop)
    pub transitive: Vec<AssetPath>,   // assets that reference target only through others (2+ hops)
}
```

`direct` and `transitive` are **mutually exclusive** — an asset that directly references
the target never also appears in `transitive`.
Neither list includes the target itself.

**Algorithm:**

1. Reverse the graph (edges point from dependency toward dependent)
2. BFS from the target node on the reversed graph
3. Nodes at depth 1 → `direct`
4. Nodes at depth 2+ → `transitive`

Use `petgraph::visit::Bfs` or a manual BFS with a visited set.

```rust
pub fn find_impact(&self, target: &AssetPath) -> ImpactResult
```

Returns empty `ImpactResult` (both Vecs empty) if `target` is not in the graph.

## Requirements

- [ ] Define `ImpactResult` struct with `direct` and `transitive` fields
- [ ] Implement `find_impact(target: &AssetPath) -> ImpactResult`
- [ ] Perform BFS on the reversed graph (`petgraph::visit::Reversed`)
- [ ] Separate depth-1 nodes into `direct`, depth-2+ into `transitive`
- [ ] Ensure no asset appears in both lists
- [ ] Return empty `ImpactResult` when target is not in the graph
- [ ] Unit test: asset with only direct references → populated `direct`, empty `transitive`
- [ ] Unit test: multi-hop chain → correct split between `direct` and `transitive`
- [ ] Unit test: target with no referencing assets → both lists empty
- [ ] Unit test: target not in graph → both lists empty (no panic)

## Related

- Depends on: #1 (DependencyGraph)
- Used by: Phase 2 Issue #8 (impact command)
- Docs: `docs/specs/crate-design.md` (ImpactResult spec)
