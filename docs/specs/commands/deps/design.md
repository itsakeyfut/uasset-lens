# `deps` Command — Internal Design

## Execution Flow

```
1. resolve_asset_path(project_dir, asset_path)  [cli]
   └── accepts game path (/Game/...) or filesystem path
   └── converts filesystem path to game path via content_root
2. AssetDb::open(db_path)                       [asset-db]
3. load_graph(&db, external_roots)              [dependency-graph]
4. Validate: graph.contains(&target) → error if not found
5. db.all_assets() → HashMap<AssetPath, (AssetType, u64)>
   └── used for type and size annotation in output
6. compute_stats(&graph, &asset_map, &target)
   └── BFS from target through forward edges
   └── direct_count  = nodes at depth 1
   └── transitive_count = nodes at depth 2+
   └── total_size = root + all reachable (visited set prevents double-counting)
7. Format output:
   └── --size-only: print stats line only (no tree)
   └── default: print tree via DFS + stats line
8. Return 0 always (informational command)
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| Asset path resolution | `uasset-lens-cli` |
| Forward edge traversal | `uasset-lens-dependency-graph` |
| DB size/type lookup | `uasset-lens-asset-db` |
| Tree rendering | `uasset-lens-cli` |

## BFS Stats Algorithm (`compute_stats`)

```rust
fn compute_stats(graph, asset_map, target) -> (direct, transitive, total_size):
    visited = HashSet { target }
    total_size = asset_map[target].size
    direct_count = 0
    transitive_count = 0
    queue: VecDeque = []

    // Depth 1: direct dependencies
    for dep in graph.dependencies_of(target):
        if visited.insert(dep):
            total_size += asset_map[dep].size
            direct_count += 1
            queue.push_back(dep)

    // Depth 2+: transitive dependencies (BFS)
    while let Some(path) = queue.pop_front():
        for dep in graph.dependencies_of(path):
            if visited.insert(dep):
                total_size += asset_map[dep].size
                transitive_count += 1
                queue.push_back(dep)
```

The `visited` set prevents double-counting in diamond dependency patterns.

## Tree Rendering (DFS)

The text tree uses recursive DFS with:
- `in_path: HashSet<AssetPath>` — tracks the current DFS path to detect cycles
- `depth: u32` and `max_depth: u32` — limits recursion; truncates with `... (N more at depth D)`
- `prefix: &str` — accumulates `│   ` / `    ` connectors for nested indentation

Cycle nodes are printed with a `[cycle]` annotation instead of recursing further.

## Depth Limiting

`--depth <N>` sets `max_depth`. When a node at depth N has children, they are
summarized as `└── ... (N more at depth N+1)` rather than omitted silently.

`--size-only` skips tree rendering entirely and outputs only the stats line:
```
Direct: 3   Transitive: 12   Total size: 45.2 MB
```
