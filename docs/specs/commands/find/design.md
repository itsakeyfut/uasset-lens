# `find` Command — Internal Design

## Execution Flow

```
1. Parse --type string → AssetType (Unknown fallback for unrecognized names)
2. AssetDb::open(db_path)                              [asset-db]
3. db.find_assets(&AssetFilter { type, min_size, max_size, path_pattern })
   └── SQL query with optional WHERE clauses
   └── path_pattern uses SQL LIKE matching
4. Lazy graph load: load_graph() only if --unreferenced, --refs, or --deps is given
5. Apply post-query filters (sequential retain() on results):
   a. --unreferenced:
      └── dead_asset_detector::detect(&graph, &[])   [dead-asset-detector]
      └── build dead: HashSet<AssetPath>
      └── retain only assets in dead set
   b. --refs <PATH>:
      └── graph.find_impact(&target) → ImpactResult  [impact-analyzer]
      └── build ref_set = direct ∪ transitive
      └── retain only assets in ref_set
   c. --deps <PATH>:
      └── graph.dependencies_of(&target)             [dependency-graph]
      └── build dep_set (direct only, no transitive)
      └── retain only assets in dep_set
6. Optional: --sort-by-size → sort descending
7. Format and output
8. Return 0 always (find is a search tool, not a gate)
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| SQL-level filtering | `uasset-lens-asset-db` |
| Unreferenced detection | `uasset-lens-dead-asset-detector` |
| Reference traversal (`--refs`) | `uasset-lens-dependency-graph` |
| Dependency lookup (`--deps`) | `uasset-lens-dependency-graph` |

## `--refs` vs `--deps` Semantic Difference

This distinction is critical and non-obvious:

| Flag | Direction | Depth |
|---|---|---|
| `--refs <X>` | Who depends on X? | **Transitive** (all ancestors) |
| `--deps <X>` | What does X depend on? | **Direct only** (depth 1) |

`--refs` uses `graph.find_impact()` which performs a full reverse BFS — it finds
every asset that transitively references the target.

`--deps` uses `graph.dependencies_of()` which returns only immediate forward
neighbors. It does **not** recurse into their dependencies.

## Filter Intersection

All active filters are applied as sequential `retain()` calls on the initial SQL
result set. Filters are AND-combined: an asset must satisfy all active filters to
appear in the output.

Example: `--type Texture2D --unreferenced --refs /Game/Materials/M_Rock` returns
only Texture2D assets that are both unreferenced AND referenced by M_Rock.
(This would typically produce an empty result — the example illustrates the AND logic.)

## Graph Lazy Loading

The dependency graph (petgraph `DiGraph`) is loaded only when at least one of
`--unreferenced`, `--refs`, or `--deps` is given. Queries using only `--type`,
`--larger-than`, `--smaller-than`, and `--path` skip graph loading entirely for
better performance.
