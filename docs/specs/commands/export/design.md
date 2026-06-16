# `export` Command — Internal Design

## Execution Flow

```
1. AssetDb::open(db_path)                              [asset-db]
2. Build filter predicate from --type, --larger-than, --smaller-than, --path
3. db.query_assets(filter)                             [asset-db]
   └── Returns Vec<AssetMetadata> sorted by path ascending
4. Build DependencyGraph from DB                       [dependency-graph]
   └── Compute deps_count = graph.out_degree(path) for each asset
   └── Compute in_degree  = graph.in_degree(path)  for each asset
5. For each asset, construct ExportRow { path, type, file_size, deps_count, in_degree }
6. Render to stdout:
   └── csv:  write header, then one row per asset
   └── json: serialize Vec<ExportRow> as JSON array
7. Exit 0
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| Asset query with filters | `uasset-lens-asset-db` |
| Degree computation | `uasset-lens-dependency-graph` |
| CSV/JSON serialization and stdout write | `uasset-lens-cli` |

## Key Data Structures

```rust
struct ExportRow {
    path:       String,   // game path
    r#type:     String,   // asset type
    file_size:  u64,
    deps_count: usize,
    in_degree:  usize,
}
```

## CSV Serialization

CSV is written manually (no external CSV crate required for five fixed columns).
The header row is hard-coded as:

```
path,type,file_size,deps_count,in_degree
```

Each data row is formatted as comma-separated values. Fields are not quoted unless
the raw value contains `,`, `"`, or `\n` (quoting uses RFC 4180 double-quote escaping).

## Performance Note

For large projects (up to 100,000 assets), the dependency graph is loaded once and
held in memory during degree computation, then freed before serialization. The full
asset list and ExportRow vec are also held in memory. At 100,000 assets with ~200
bytes per row, peak memory usage is approximately 20 MB — within the 100 MB budget.

## Filter Interaction with `find`

The `--type`, `--larger-than`, `--smaller-than`, and `--path` filters reuse the same
`AssetFilter` struct already used by the `find` command. No new filtering logic is
introduced in the export path.

## Stdout vs File Output

`export` always writes to stdout. Callers redirect to a file using shell redirection
(`> assets.csv`). The command does not provide a `--output` flag. This keeps the
interface composable with Unix pipelines.
