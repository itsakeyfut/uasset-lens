# `history` Command — Internal Design

## Execution Flow

```
1. Resolve history_dir = <project_dir>/.uasset-lens/history/
2. If history_dir does not exist → print "No scan history recorded." → exit 0
3. Read directory entries; collect *.json filenames
4. Sort filenames descending (ISO 8601 filenames sort correctly lexicographically)
5. Apply --limit: take first N entries
6. For each filename:
   a. Read file contents
   b. Deserialize into SnapshotV1
   c. On parse error: log WARN to stderr, skip entry
7. Assign index 1..=N (1 = newest)
8. Render output (text table or JSON) to stdout
9. Exit 0
```

## Crate Responsibilities

| Step | Crate |
|---|---|
| History directory resolution | `uasset-lens-cli` |
| Snapshot file I/O and deserialization | `uasset-lens-cli` (history/snapshot.rs) |
| Rendering | `uasset-lens-cli` |

## Snapshot Write (scan integration)

The `scan` command writes a snapshot at step 16 of its execution flow. The write is
best-effort: a failure to write the snapshot is logged as a warning but does not cause
`scan` to exit with code `2`.

```rust
// Written by scan after db.record_scan_snapshot()
struct SnapshotV1 {
    version:      u32,           // always 1
    scanned_at:   DateTime<Utc>,
    git_commit:   Option<String>,
    assets_total: u64,
    new:          u64,
    updated:      u64,
    removed:      u64,
}
```

The filename is `<scanned_at formatted as RFC 3339 with colons replaced by dashes>.json`
to ensure filesystem compatibility on Windows (e.g. `2026-06-16T14-23-01Z.json`).
When reading, the command strips the extension and parses the timestamp from the file
contents, not the filename.

## Git Commit Detection

At scan time, the CLI runs `git rev-parse --short HEAD` in the project directory.
If the command fails (not a git repo, git not installed), `git_commit` is set to `null`.

## Column Alignment

Text output uses fixed-width columns. `Date` is formatted as `YYYY-MM-DD HH:MM` in
local time. All numeric columns are right-aligned within their column width.
