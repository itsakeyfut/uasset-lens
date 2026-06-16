# `history` Command — Specification

## Purpose

Display a chronological list of past scan runs stored in `.uasset-lens/history/`.
Each completed `scan` writes a snapshot file; `history` reads and renders them.

```bash
uasset-lens history ./Project
uasset-lens history ./Project --limit 10
```

---

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | History displayed (including the empty case) |
| `2` | Execution error (I/O failure, malformed snapshot file) |

---

## Snapshot Storage

Each scan completion writes a JSON file to:

```
<project_dir>/.uasset-lens/history/<ISO8601-timestamp>.json
```

Example filename: `2026-06-16T14:23:01Z.json`

Snapshot schema (version 1):

```json
{
  "version": 1,
  "scanned_at": "2026-06-16T14:23:01Z",
  "git_commit": "abc1234",
  "assets_total": 1024,
  "new": 3,
  "updated": 5,
  "removed": 0
}
```

Fields:
- `version`: schema version, currently `1`
- `scanned_at`: RFC 3339 timestamp of when the scan completed
- `git_commit`: short SHA of `HEAD` at scan time, or `null` if not in a git repo
- `assets_total`: total assets in the DB after the scan
- `new`: assets added this scan
- `updated`: assets re-parsed due to mtime change
- `removed`: stale records deleted this scan

---

## Text Output

```
Scan History (./Project)

#  Date                 Assets  New  Updated  Removed  Commit
1  2026-06-16 14:23     1,024   3    5        0        abc1234
2  2026-06-15 09:11     1,021   0    12       1        def5678
3  2026-06-14 16:45     1,022   8    0        0        ghi9012

3 entries
```

Entries are listed newest-first (index 1 = most recent). When `--limit` is set, only
the N most recent entries are shown. The footer always reflects the number of entries
displayed, not the total in storage.

When no history files exist:

```
No scan history recorded.
```

When `git_commit` is `null`, the `Commit` column shows `—`.

---

## JSON Output (`--format json`)

```json
{
  "project": "./Project",
  "total_entries": 3,
  "entries": [
    {
      "index": 1,
      "scanned_at": "2026-06-16T14:23:01Z",
      "assets_total": 1024,
      "new": 3,
      "updated": 5,
      "removed": 0,
      "git_commit": "abc1234"
    },
    {
      "index": 2,
      "scanned_at": "2026-06-15T09:11:00Z",
      "assets_total": 1021,
      "new": 0,
      "updated": 12,
      "removed": 1,
      "git_commit": "def5678"
    }
  ]
}
```

`total_entries` reflects the number returned (after applying `--limit`), not the total
files on disk.

---

## Error Cases

| Condition | Behaviour |
|---|---|
| History directory missing | Treat as empty; print "No scan history recorded." |
| Snapshot file is malformed JSON | Log warning to stderr, skip that entry, continue |
| History directory unreadable (permissions) | Exit `2` with descriptive error |
