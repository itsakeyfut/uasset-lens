# `watch` Command — Specification

## Purpose

Watch the project directory for file changes and print new problems as assets are
modified. Runs an initial scan on startup, then keeps running until interrupted with
Ctrl+C.

```bash
uasset-lens watch ./Project
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Normal exit (Ctrl+C) |
| `2` | Execution error (initial scan failed, DB error, etc.) |

---

## Behavior

1. **Initial scan**: Runs a full mtime delta scan on startup to bring the DB up to date.
2. **Watch loop**: Monitors the Content directory for `.uasset` / `.umap` file events
   (create, modify, delete) using the OS file system notification API.
3. **Debounce**: Events are debounced to avoid running on every intermediate save during
   a save operation.
4. **Re-analysis**: On each file change event, the affected asset is re-scanned and a
   quick health check is run. Results are printed to stdout immediately.
5. **Shutdown**: Ctrl+C terminates the watch loop cleanly.

---

## Text Output

```
$ uasset-lens watch ./Project

Initial scan: 1,024 assets indexed (1.2s)
Watching ./Project/Content... (press Ctrl+C to stop)

[12:34:01] Modified: Content/Characters/BP_Player.uasset
  ⚠ blueprint: EventTick node count (9) exceeds limit (5)

[12:35:22] Modified: Content/Materials/M_Rock.uasset
  ✓ no issues

[12:36:45] Created: Content/Unused/T_Test.uasset
  ℹ dead-asset: no incoming references

^C
Watch stopped.
```

---

## Cycle Detection Events

When a file change creates or resolves a dependency cycle, `watch` emits a specific
event line:

```
[12:37:10] Modified: Content/Characters/BP_Enemy.uasset
  🔴 NEW CYCLE: BP_Player → BP_Enemy → BP_GameMode → BP_Player
```

```
[12:38:55] Modified: Content/GameModes/BP_GameMode.uasset
  ✅ CYCLE RESOLVED: BP_Player → BP_Enemy → BP_GameMode → BP_Player
```

Cycle detection runs after every asset change by re-evaluating the affected subgraph.

---

## Notes

- `watch` does not support `--format json` or `--format github-actions`. Output is
  always text, as it is an interactive continuous stream.
- The command does not block on confirmation prompts. Any prompt-requiring operations
  (e.g., stale record cleanup) are handled automatically during the initial scan.
- Cycle detection on each change may be slow for large projects (>50k assets).
  Use `--no-cycle-check` to disable it if latency is a concern.
