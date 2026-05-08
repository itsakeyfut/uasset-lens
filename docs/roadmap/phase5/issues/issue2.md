# `crates/watcher` — scan integration and problem notification

## Summary

Extend the watcher to re-scan changed files via `scanner::scan_files()`, update the DB,
and report newly detected problems (new dead assets, new cycles).
Complete when a file change triggers a re-scan and a new dead asset is printed to stdout.

## Design Notes

**Re-scan flow (triggered per debounced batch):**

```
WatchEvent { kind: Changed/Created, paths }
  └─ scanner::scan_files(&paths, content_root) → ScanResult
  └─ rusqlite::Transaction { upsert_asset + replace_dependencies for each }
  └─ load_graph(db)
  └─ dead_asset_detector::detect(&graph) → compare with previous dead set
  └─ dependency_graph::find_cycles() → compare with previous cycle set
  └─ print newly added problems to stdout
```

For `Deleted` events: call `db.delete_asset()` for each path.

**Previous state tracking:** store the last-known dead asset set and cycle set in the
`WatchSession` struct. Diff against the new results after each re-scan.

```rust
pub struct WatchSession {
    db: AssetDb,
    content_root: PathBuf,
    last_dead: HashSet<AssetPath>,
    last_cycles: Vec<Vec<AssetPath>>,
}

impl WatchSession {
    pub fn run(watcher: Watcher, db: AssetDb, content_root: PathBuf) -> Result<()>
}
```

`run()` loops on `watcher.next_batch()` until interrupted by Ctrl+C signal.

## Requirements

- [ ] Define `WatchSession` struct
- [ ] Implement `WatchSession::run()` loop calling `watcher.next_batch()`
- [ ] On Changed/Created: re-scan changed paths, upsert to DB, detect problems
- [ ] On Deleted: call `db.delete_asset()` for each deleted path
- [ ] Compare new dead asset set with `last_dead`; print newly added paths
- [ ] Compare new cycle set with `last_cycles`; print newly added cycles
- [ ] Handle Ctrl+C gracefully (return from `run()` cleanly)
- [ ] Unit test: mock watcher with injected events → re-scan triggered, new dead asset printed

## Related

- Depends on: #1 (Watcher core), Phase 1 Issues #8, #10 (scan_files, DB write)
- Used by: #4 (watch command)
