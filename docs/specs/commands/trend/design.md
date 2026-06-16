# `trend` Command — Internal Design

## Execution Flow

```
1. Resolve history_dir = <project_dir>/.uasset-lens/history/
2. Read and deserialize all SnapshotV1 files (same as history command)
3. Sort descending by scanned_at; apply --limit
4. If fewer than 2 valid entries → print insufficient-data message → exit 0
5. AssetDb::open(db_path)                              [asset-db]
   └── needed for dead-assets and cycles columns
6. Load baselines index:
   └── Read .uasset-lens/baselines/*.json
   └── Build map: date_bucket → error_count
   └── "date bucket" = scanned_at truncated to the minute for fuzzy matching
7. For each history entry, build TrendRow:
   a. assets:          snapshot.assets_total
   b. file_size_total: snapshot.file_size_total (if present in snapshot; else query DB)
   c. violations:      lookup nearest baseline by date (within 5-minute window); None → None
   d. dead_assets:     db.count_dead_assets_at(snapshot.scanned_at) if available; else None
   e. cycles:          db.count_cycles_at(snapshot.scanned_at) if available; else None
8. Compute trend delta: newest_row − oldest_row for each metric (skip Nones)
9. Render output (text table or JSON) to stdout
10. Exit 0
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| History file I/O | `uasset-lens-cli` (shared with history command) |
| Baseline index loading | `uasset-lens-cli` (shared with baseline command) |
| Dead-asset count per snapshot | `uasset-lens-asset-db` |
| Cycle count per snapshot | `uasset-lens-asset-db` |
| TrendRow assembly and rendering | `uasset-lens-cli` |

## Key Data Structures

```rust
struct TrendRow {
    date:            DateTime<Utc>,
    assets:          Option<u64>,
    violations:      Option<u64>,
    dead_assets:     Option<u64>,
    cycles:          Option<u64>,
    file_size_total: Option<u64>,
}

struct TrendDelta {
    metric:    String,
    delta:     i64,      // signed; positive = increased
    direction: Direction,
}

enum Direction { Up, Down, Unchanged }
```

## Violations Column Data Source

The `violations` column requires a saved baseline to have been written near the time
of each scan. The command does a fuzzy date match: for each history entry, it finds
the baseline with `saved_at` within 5 minutes of `scanned_at`. If no match is found,
the column value is `None` (rendered as `—` in text, `null` in JSON).

This is why the specification notes that running `baseline save` after each scan
produces the best `violations` trend data.

## Historical Dead-Asset and Cycle Counts

The DB stores current-state data only; it does not retain per-scan snapshots of dead
assets or cycles. Therefore, for entries other than the most recent scan, `dead_assets`
and `cycles` will typically be `None` (shown as `—`). Future work may add per-scan
counters to the snapshot file to resolve this.

The most recent entry always has live values because the DB reflects the latest scan.

## File Size Rendering

`file_size_total` is formatted as GB in text output with one decimal place
(e.g., `4.2 GB`). The trend delta summary uses the same unit. In JSON, the raw byte
count is emitted as an integer.
