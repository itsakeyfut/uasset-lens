# `blueprint` Command — Internal Design

## Execution Flow

```
1. AssetDb::open(db_path)                        [asset-db]
2. db.all_blueprint_metrics()                    [asset-db]
   └── returns Vec<BlueprintMetricsRow>
   └── only assets with blueprint_metrics IS NOT NULL are returned
3. Sort by node_count DESC
4. Map to Vec<BlueprintEntry>
5. Format and output:
   └── text: rank table (rank, path, nodes, ticks, casts, depth)
   └── json: array of BlueprintEntry objects
   └── github-actions: one ::notice annotation per asset
6. Return 0 always (informational ranking)
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| Metrics storage | `uasset-lens-asset-db` |
| Metrics extraction from binary | `uasset-lens-scanner` |

## BlueprintMetrics Source

Blueprint complexity metrics are extracted during scan by `uasset-lens-scanner` when
parsing Blueprint-family assets. The parser reads the KismetBytecode section and counts:

- `node_count` — total K2 nodes in the event graph
- `event_tick_count` — number of `Event Tick` entry points (runtime performance risk)
- `cast_count` — number of object cast nodes (coupling indicator)
- `dependency_depth` — longest dependency chain depth for this Blueprint

These are stored in the `blueprint_metrics` table and returned by `all_blueprint_metrics()`.
Assets without Blueprint bytecode (e.g. non-Blueprint `.uasset` files) have no row and
do not appear in this command's output.

## github-actions Format

In `--format github-actions` mode, each Blueprint is emitted as a `::notice` annotation
(not `::warning` or `::error`) because the output is a report, not a gate. Unlike `lint`
or `budget`, this command does not exit 1 on high complexity — it surfaces metrics for
human review.
