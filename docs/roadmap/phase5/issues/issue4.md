# `crates/cli` — `watch` command

## Summary

Implement the `watch` command that starts a file system watch session, performing
an initial scan on startup and re-scanning on changes.
Complete when `uasset-lens watch ./Project` runs continuously, printing new problems
as files change, and exits cleanly on Ctrl+C.

## Design Notes

**Startup sequence:**

```
1. resolve DB path + content root
2. uasset-lens scan ./Project  (initial full scan, same as running scan command)
3. Print "Watching for changes. Press Ctrl+C to stop."
4. WatchSession::run(watcher, db, content_root)  (blocks until Ctrl+C)
```

**Output during watch:**

```
[12:34:56] Changed: /Game/Characters/BP_Player.uasset
  → Rescanned 1 file (0 skipped)
  ⚠ New dead asset: /Game/UI/WBP_HUD

[12:35:10] Deleted: /Game/Textures/T_Old.uasset
  → Removed from index
```

Ctrl+C handling: use `ctrlc` crate or `signal-hook` to set an atomic flag that
`WatchSession::run()` checks each iteration.

## Requirements

- [ ] Implement `watch` command handler
- [ ] Run initial scan (reuse scan command logic) before entering watch loop
- [ ] Construct `Watcher` and `WatchSession`, call `WatchSession::run()`
- [ ] Print timestamp + changed file path on each event
- [ ] Print re-scan result and any new problems after each batch
- [ ] Handle Ctrl+C: exit `run()` cleanly, print `"Watch stopped."` before exit
- [ ] Add `ctrlc` to `[workspace.dependencies]` for signal handling

## Related

- Depends on: #2 (WatchSession::run)
