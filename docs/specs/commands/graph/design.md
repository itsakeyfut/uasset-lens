# `graph` Command — Internal Design

## Execution Flow

```
1. AssetDb::open(db_path)                [asset-db]
2. load_graph(&db, external_roots)       [dependency-graph]
   └── all_assets() + all_edges() → DependencyGraph::build()
3. graph.find_cycles()                   [dependency-graph]
   └── Tarjan SCC algorithm
   └── returns Vec<Vec<AssetPath>>  (one inner Vec per cycle)
4. graph.nodes().count() → total_assets
5. graph.edge_count()   → total_edges
6. Build closed-path representation:
   └── for each SCC: append first node at end  (A→B→A visual)
7. Format and output
8. Return exit code:
   └── --cycles-only + cycles present → 1
   └── all other cases → 0
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| Graph construction | `uasset-lens-dependency-graph` |
| Cycle detection (Tarjan SCC) | `uasset-lens-dependency-graph` |
| Output formatting | `uasset-lens-cli` |

## Cycle Detection Algorithm

`graph.find_cycles()` runs Tarjan's Strongly Connected Components (SCC) algorithm on the
petgraph `DiGraph`. Any SCC with more than one node is a cycle. The algorithm runs in O(V+E).

Each returned cycle is a `Vec<AssetPath>` containing the unique nodes in the SCC. The CLI
layer appends the first node at the end to produce a closed-path visual:

```
A → B → C → A
```

## Cycle Truncation

Long cycles (> 6 unique nodes) are truncated in text output to avoid terminal noise:

```
/Game/N0 → /Game/N1 → ... (5 nodes) → /Game/N0
```

`--full-cycles` disables truncation and shows all nodes. This threshold is controlled by
`CYCLE_TRUNCATE_THRESHOLD = 6` in `graph.rs`.

## Exit Code Semantics

The default mode is informational: cycles are reported but exit code is always 0.

`--cycles-only` switches to CI gate mode: exit 1 if any cycles exist, exit 0 if none.
This is the intended mode for CI pipelines.
