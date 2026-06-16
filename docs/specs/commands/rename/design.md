# `rename` Command — Internal Design

## Execution Flow

```
1. AssetDb::open(db_path)                              [asset-db]
2. db.get_asset(source_path)                           [asset-db]
   └── AssetNotFound → exit 2 with error + hint
3. Check if destination_path already exists in DB
   └── If found: print warning line to stderr; continue
4. DependencyGraph::from_db(&db)                       [dependency-graph]
5. Compute direct_refs:
   └── impact_analyzer::find_direct_referrers(graph, source_path)
       OR graph.direct_referrers(source_path)
   └── Returns Vec<AssetRef> { path, type }
6. Compute transitive_impact_count:
   └── impact_analyzer::find_impact(graph, source_path)  [impact-analyzer]
   └── transitive_count = find_impact result count − direct_refs count
7. Render output to stdout
8. Exit 1 if direct_refs non-empty, else exit 0
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| Asset existence check | `uasset-lens-asset-db` |
| Dependency graph construction | `uasset-lens-dependency-graph` |
| Direct referrer lookup | `uasset-lens-dependency-graph` |
| Transitive impact computation | `uasset-lens-impact-analyzer` |
| Rendering | `uasset-lens-cli` |

## Impact Computation Detail

`impact_analyzer::find_impact(graph, source_path)` performs a reverse BFS/DFS from
`source_path`, collecting all ancestors in the reference graph (all assets that
transitively depend on the source). This returns the full transitive closure.

```
direct_refs          = set of assets with a direct edge → source_path
transitive_all       = find_impact(source_path)
transitive_indirect  = transitive_all − direct_refs
transitive_impact_count = |transitive_indirect|
```

Only `direct_refs` assets are listed in the output; the transitive count is reported
as a number to avoid overwhelming output for high-fanout assets.

## Destination Exists Warning

When the destination path already exists in the DB (the name is already taken), the
command prints a warning on stderr but does not abort:

```
warning: /Game/Characters/BP_NewName already exists in the database
```

The simulation continues because the user may be planning to delete the destination
asset first. Aborting here would make the command less useful for planning purposes.

## Read-Only Guarantee

`rename` never calls any DB write methods. It opens the DB in read-only mode where
the underlying rusqlite connection allows, or uses only read queries if read-only mode
is unavailable. No files on disk are touched.
