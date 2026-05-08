# `crates/dependency-graph` — `find_cycles()`

## Summary

Implement `find_cycles()` on `DependencyGraph` using Tarjan's strongly connected
components algorithm.
Complete when cycles in fixture graphs are detected correctly and single-node SCCs
(self-references) are excluded.

## Design Notes

**Algorithm:** use `petgraph::algo::tarjan_scc()`.

`tarjan_scc` returns all SCCs including trivial ones (single nodes with no self-loop).
Filter the result to keep only SCCs with **2 or more nodes** — these represent true circular
dependencies.

**Return format:** `Vec<Vec<AssetPath>>`.
Each inner `Vec` is one cycle, expressed as the path set that forms the SCC.
Order within the inner Vec is not required to be deterministic — callers should not rely on it.

```rust
pub fn find_cycles(&self) -> Vec<Vec<AssetPath>>
```

**Test cases to cover:**

| Graph | Expected |
|---|---|
| DAG (no cycles) | empty Vec |
| A → B → A | one cycle: [A, B] |
| A → B → C → A | one cycle: [A, B, C] |
| Two independent cycles | two entries in outer Vec |
| Self-loop A → A only | excluded (single-node SCC) |

## Requirements

- [ ] Implement `find_cycles(&self) -> Vec<Vec<AssetPath>>`
- [ ] Use `petgraph::algo::tarjan_scc()` internally
- [ ] Filter out SCCs with fewer than 2 nodes
- [ ] Map `NodeIndex` back to `AssetPath` via the graph node weights
- [ ] Unit test: DAG → empty result
- [ ] Unit test: 2-node mutual reference → one cycle returned
- [ ] Unit test: 3-node cycle → one cycle returned
- [ ] Unit test: two independent cycles → two entries returned
- [ ] Unit test: self-loop only → excluded from results

## Related

- Depends on: #1 (DependencyGraph, build())
- Next: #3 — `find_impact()`
- Used by: Phase 2 Issue #6 (graph command, `--cycles-only`)
