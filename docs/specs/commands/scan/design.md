# `scan` Command — Internal Design

## Execution Flow

```
1. create_dir_all(db_path.parent)        [cli]
2. AssetDb::open(db_path)                [asset-db]
3. Fail-fast: load baseline if --diff-from given
   └── DbError::BaselineNotFound → user-visible error with hint  [asset-db]
4. resolve_content_root(project_dir)     [cli]
5. Snapshot DB: db.all_known_files()     [asset-db]
   └── db_files: HashSet<PathBuf>  (used for new/updated/stale classification)
6. If --diff or --diff-from: capture pre-scan metrics
   └── old_bp: HashMap<AssetPath, (node_count, event_tick_count)>
   └── old_sizes: HashMap<AssetPath, file_size>
7. WalkDir(project_dir) with exclusion filter
   └── skip dirs matching scan.exclude_paths (normalized to forward slashes)
   └── keep only .uasset / .umap files (case-insensitive)
   └── collect (PathBuf, mtime_secs) pairs
8. Compute stale = db_files − walkdir_paths  (set difference)
9. Compute paths_to_scan:
   └── --full-scan: all walkdir files
   └── default:  db.filter_changed(all_files)  [asset-db]
      → SQL query: compare stored mtime vs current mtime
10. Print scan header to stderr
11. scanner::scan_files(paths_to_scan, content_root)  [scanner, rayon]
    └── parallel parse; returns ScanResult { assets, skipped }
12. Classify new vs updated against db_files snapshot
13. db.upsert_all(result.assets)          [asset-db]  (single transaction)
14. Print per-category counts to stdout
15. If stale records exist:
    └── prompt user [y/N] (skip if --yes)
    └── db.delete_asset(ap) for each confirmed stale record
16. db.record_scan_snapshot()             [asset-db]
17. If --save-baseline: db.save_baseline(name, snapshot_id)
18. If --diff or --diff-from: diff::print_diff(...)
    └── compare current scan vs previous snapshot or named baseline
    └── flags size increases > threshold (default 20%) as regressions
19. Print final summary; return exit code
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| File discovery (WalkDir) | `uasset-lens-cli` |
| mtime delta filtering | `uasset-lens-asset-db` (`filter_changed`) |
| Binary parsing (parallel) | `uasset-lens-scanner` + `rayon` |
| DB persistence | `uasset-lens-asset-db` |
| Baseline storage | `uasset-lens-asset-db` |
| Diff report | `uasset-lens-cli` (scan/diff.rs) |

## Key Data Structures

```rust
// Pre-scan DB snapshot (step 5) — used to classify new vs updated
let db_files: HashSet<PathBuf> = db.all_known_files()?;

// WalkDir output (step 7)
let all_files: Vec<(PathBuf, u64)>;  // (path, mtime_secs)

// Stale detection (step 8)
let stale: Vec<PathBuf> = db_files.iter()
    .filter(|p| !walkdir_paths.contains(p))
    .collect();

// ScanResult from scanner (step 11)
struct ScanResult {
    assets: Vec<AssetMetadata>,
    skipped: Vec<SkippedFile>,
}
```

## mtime Delta Algorithm

`db.filter_changed(all_files)` issues a SQL query that loads stored mtimes for known paths
and returns only files whose current mtime differs from the stored value. This is the primary
mechanism that makes repeated scans fast: only changed files are re-parsed.

Full scan (`--full-scan`) bypasses this and passes all discovered files to the parser.

## Baseline Storage Model

```
scan_history table: (id, scanned_at, asset_count)
baselines table:    (name, snapshot_id)  → FK to scan_history.id
```

`--save-baseline <name>` records the current snapshot_id under a human-readable name.
`--diff-from <name>` loads the baseline snapshot to compare metrics against, rather than
using the most recent previous scan.

## Stale Record Handling

A stale record exists when a file is in the DB but no longer on disk. Detected by set
difference (`db_files − walkdir_paths`). By default, the user is prompted before removal.
`--yes` skips the prompt. Exit code 1 is returned when stale records are removed.

## Parallelism Note

`scanner::scan_files` uses `rayon::par_iter` over the list of changed files. DB writes
happen sequentially after the parallel parse phase (`upsert_all` wraps all rows in a
single transaction for performance).
