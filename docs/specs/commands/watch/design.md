# `watch` Command — Internal Design

## Execution Flow

```
1. create_dir_all(db_path.parent)        [cli]
2. Initial scan (auto-yes, non-interactive):
   └── scan::handle_scan(project_dir, ..., yes=true)   [cli/scan]
   └── Populates DB before the watch loop starts
3. resolve_content_root(project_dir)     [cli]
4. AssetDb::open(db_path)               [asset-db]
5. config::load_config(project_dir)     [cli]
   └── Re-read from disk so edits during the session take effect
6. Install Ctrl+C handler:
   └── Arc<AtomicBool> stop flag
   └── ctrlc::set_handler → store(true, Relaxed)
   └── MultipleHandlers error (in tests) is logged as warning, not fatal
7. Watcher::new(project_dir)            [watcher]
   └── notify crate filesystem watcher on Content/
8. WatchSession::new(db, content_root, external_roots)  [watcher]
9. session.init()                        [watcher]
10. Event loop (until stop == true):
    a. if stop.load(Relaxed): break
    b. watcher.next_batch_timeout(200ms) → Option<Vec<WatchEvent>>
    c. if Some(batch):
       └── print_batch_header(batch)  → [HH:MM:SS] Changed/Created/Deleted: <path>
       └── session.process_batch(&batch, &mut stdout)  [watcher]
           → re-scan changed files
           → update DB
           → run quick health checks on affected assets
           → output results
11. eprintln!("Watch stopped."); return 0
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| File system notification | `uasset-lens-watcher` (notify crate) |
| Debounced event batching | `uasset-lens-watcher` |
| DB update on change | `uasset-lens-watcher` + `uasset-lens-asset-db` |
| Quick health checks | `uasset-lens-watcher` |
| Timestamp formatting | `uasset-lens-cli` |

## Debounce Mechanism

`watcher.next_batch_timeout(Duration::from_millis(200))` blocks for up to 200 ms
collecting filesystem events before returning a batch. This prevents re-processing
the same file multiple times when an editor writes an asset in multiple syscalls
(e.g. truncate + write + metadata update).

Events within the 200 ms window are coalesced into a single batch.

## `WatchSession`

`WatchSession` owns the DB handle and content root for the duration of the watch
session. On each batch it:

1. Re-parses the changed `.uasset` / `.umap` files
2. Upserts the updated records into the DB
3. Runs a lightweight subset of health checks on the affected assets
4. Writes results to stdout

The full `check` suite is not run on each event — only checks that are fast enough
to complete within a reasonable interactive latency (< 1 s).

## Ctrl+C Shutdown

The stop signal uses `Arc<AtomicBool>` with `Ordering::Relaxed`. The main loop
checks it at the top of each iteration (before blocking on `next_batch_timeout`),
so shutdown completes within at most 200 ms after Ctrl+C.

## Timestamp Format

Event timestamps are printed in UTC using `SystemTime::now()` without the `chrono`
dependency. Local timezone is not available in `std`, so UTC is used intentionally.
Format: `HH:MM:SS`.
