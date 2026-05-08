# `crates/dependency-graph` — core types, `build()`, `nodes()`, and `in_degree()`

## Summary

Create the `dependency-graph` crate with its core type definitions and the foundational
graph construction API.
Complete when a `DependencyGraph` can be built from a list of nodes and edges and
`in_degree()` returns correct values.

## Design Notes

**Types:**

```rust
pub struct AssetNode {
    pub path: AssetPath,
    pub asset_type: AssetType,
}

pub struct DependencyGraph {
    graph: DiGraph<AssetNode, ()>,
    index: HashMap<AssetPath, NodeIndex>,
}
```

Use `petgraph::graph::DiGraph`. Edges point **from** a dependent asset **to** its dependency
(i.e., `A → B` means A depends on B).

**`build(nodes, edges)` contract:**
1. Add all `AssetNode` entries to the graph first
2. For each `(from_path, to_path)` edge: if either path is not in `nodes`, create a placeholder `AssetNode` with `asset_type = AssetType::Unknown("".into())` rather than dropping the edge
3. Add the directed edge

**`in_degree(path)`:** number of edges pointing *into* the node for `path` (i.e., how many assets reference it). Use `petgraph::Direction::Incoming`.

`dependency-graph` depends only on `shared` and `petgraph`. No DB or IO.

## Requirements

- [ ] Create `crates/dependency-graph` crate with `petgraph` dependency
- [ ] Define `AssetNode` struct
- [ ] Define `DependencyGraph` struct with `DiGraph<AssetNode, ()>` and `HashMap<AssetPath, NodeIndex>` index
- [ ] Implement `DependencyGraph::build(nodes: Vec<AssetNode>, edges: Vec<(AssetPath, AssetPath)>) -> Self`
- [ ] Implement `nodes() -> impl Iterator<Item = &AssetNode>`
- [ ] Implement `in_degree(path: &AssetPath) -> usize`
- [ ] Unit test: `build()` with isolated nodes (no edges) → all nodes present
- [ ] Unit test: `build()` with edges where `to_path` is not in `nodes` → placeholder node created
- [ ] Unit test: `in_degree()` returns 0 for unreferenced node, correct count for multi-referenced node

## Related

- Next: #2 — `find_cycles()`
- Used by: Phase 2 Issue #3 (find_impact), Phase 2 Issue #4 (dead-asset-detector)
- Docs: `docs/roadmap/phase2/ROADMAP.md` — Task 1, `docs/specs/crate-design.md`
