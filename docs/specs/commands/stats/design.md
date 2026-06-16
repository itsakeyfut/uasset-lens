# `stats` Command — Internal Design

## Execution Flow

```
1. AssetDb::open(db_path)                      [asset-db]
2. db.all_assets() → Vec<AssetRecord>          [asset-db]
3. load_graph(&db, external_roots)             [dependency-graph]
4. Compute limits from --top:
   └── None       → type_limit=10, folder_limit=5, asset_limit=10
   └── Some(0)    → unlimited (usize::MAX)
   └── Some(N)    → N for all three
5. Aggregate by_type:
   └── HashMap<String, (count, bytes)>
   └── sort by bytes DESC, truncate to type_limit
6. Aggregate by_folder:
   └── path_depth_prefix(asset_path) → first 3 path segments
   └── HashMap<String, bytes>
   └── sort by bytes DESC, truncate to folder_limit
7. Sort all_assets by file_size DESC, take asset_limit → largest list
8. Compute graph statistics:
   └── total_edges = graph.edge_count()
   └── avg_out_degree = total_edges / total_assets
   └── unreferenced = count of nodes where graph.in_degree(&path) == 0
   └── cycle_count = graph.find_cycles().len()
9. Format and output
10. Return 0 always (informational)
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| Asset enumeration | `uasset-lens-asset-db` |
| Graph metrics | `uasset-lens-dependency-graph` |
| Aggregation and display | `uasset-lens-cli` |

## `path_depth_prefix` Bucketing

Folders are bucketed by taking the first 3 segments of the game path:

```
/Game/Characters/Enemies/BP_Goblin → /Game/Characters/Enemies/
/Game/UI/HUD/WBP_Health            → /Game/UI/HUD/
/Game/Effects/T_Fire               → /Game/Effects/
```

This gives a meaningful top-level view without exposing every leaf directory.
The function is shared with `dead-assets --group dir`.

## `--top` Behavior

`--top 0` means "show all" for all three limits. This is used when the user wants
a complete audit rather than a sampled view. The default limits (10/5/10) are
chosen to fit in a single terminal screen for typical projects.

## Graph Statistics

Graph stats are computed from the already-loaded dependency graph rather than
issuing additional DB queries:

- `avg_out_degree`: mean number of direct dependencies per asset (edges / nodes)
- `unreferenced`: assets with `in_degree == 0` (no other asset references them)
- `cycles`: number of distinct SCCs with >1 node (from Tarjan SCC)

These values are informational and do not affect the exit code.
