# `redirectors` Command — Internal Design

## Execution Flow

```
1. AssetDb::open(db_path)                    [asset-db]
2. load_graph(&db, external_roots)           [dependency-graph]
3. redirector_analyzer::detect(&graph)       [redirector-analyzer]
   └── filters graph nodes by AssetType::ObjectRedirector
   └── returns Vec<AssetPath>
4. Format and output
5. Return: 1 if any redirectors found, 0 if none
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| ObjectRedirector detection | `uasset-lens-redirector-analyzer` |
| Graph traversal | `uasset-lens-dependency-graph` |

## Detection Logic

`redirector_analyzer::detect()` walks the graph nodes and collects all paths where
`node.asset_type == AssetType::ObjectRedirector`. The graph is used (rather than
querying the DB directly) to keep the analysis layer consistent with other analyzers.

An `ObjectRedirector` is a UE5 stub asset that forwards asset references from a
renamed or moved source to the new location. They accumulate after rename operations
and should be resolved (i.e., references updated and the redirector deleted) before
shipping.

## Why Graph, Not Direct DB Query

The redirector analyzer uses the dependency graph API rather than querying the DB
directly. This ensures that `external_roots` filtering is applied consistently —
redirectors reachable only through excluded roots are still surfaced, as they exist
as standalone files regardless of reachability.
