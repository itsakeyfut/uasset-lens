# `crates/watcher` — file system watcher core

## Summary

Create the `watcher` crate with `notify`-based file system monitoring for
`.uasset` / `.umap` files, including debounce processing.
Complete when file creation and modification events for `.uasset` files are
detected and debounced correctly in unit tests.

## Design Notes

**Library:** use the `notify` crate (cross-platform file system events).

**Event flow:**

```
notify::RecommendedWatcher
  └─ raw events (Create, Modify, Remove)
  └─ filter: only .uasset and .umap extensions
  └─ debounce: accumulate events for 300 ms, then emit a single batch

WatchEvent { kind: WatchEventKind, paths: Vec<PathBuf> }

pub enum WatchEventKind { Changed, Created, Deleted }
```

**Debounce:** use a `std::sync::mpsc` channel internally.
Collect events for 300 ms using `recv_timeout`, then flush the accumulated set.
Multiple changes to the same path within the debounce window produce one event.

**`Watcher` struct:**

```rust
pub struct Watcher {
    _watcher: notify::RecommendedWatcher,
    rx: Receiver<Vec<WatchEvent>>,
}

impl Watcher {
    pub fn new(project_dir: &Path) -> Result<Self>
    pub fn next_batch(&self) -> Option<Vec<WatchEvent>>  // blocks until next debounced batch
}
```

**Testing:** inject synthetic events through a channel rather than hitting the real file system.

## Requirements

- [ ] Add `notify` to `[workspace.dependencies]`
- [ ] Create `crates/watcher` crate
- [ ] Define `WatchEvent` and `WatchEventKind`
- [ ] Implement `Watcher::new(project_dir) -> Result<Self>` using `notify::RecommendedWatcher`
- [ ] Filter events to `.uasset` and `.umap` extensions only
- [ ] Implement 300 ms debounce: multiple changes to the same path within the window → one event
- [ ] Implement `next_batch()` returning the debounced batch (blocking)
- [ ] Unit test: synthetic events for same path within 300 ms → deduplicated to one event
- [ ] Unit test: non-`.uasset` file changes → excluded from batch

## Related

- Next: #2 — watcher integration with scan_files
- Used by: #4 (watch command)
