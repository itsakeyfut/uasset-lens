# `crates/cli` — `scan` command: walkdir, differential scan, and batch upsert

## Summary

Implement the core logic of the `scan` command: enumerating `.uasset`/`.umap` files,
performing a differential scan via `filter_changed()`, parsing changed files with
`scan_files()`, and batch-upserting the results into SQLite.
Complete when `uasset-lens scan ./Project` indexes all assets into `uasset-lens.db`.

## Design Notes

**Scan flow:**

```
walkdir <project_dir>
  └─ filter: extension == .uasset or .umap
  └─ collect (PathBuf, mtime_secs) pairs

db.filter_changed(&files)
  └─ returns only new/modified paths

scanner::scan_files(&changed_paths, &content_root)
  └─ returns ScanResult { assets, skipped }

rusqlite::Transaction
  └─ for each asset: db.upsert_asset(&meta) → id
  └─                 db.replace_dependencies(id, &meta.dependencies)
  └─ commit
```

**`--full-scan` flag:** skip `filter_changed()` entirely and pass all collected paths to `scan_files()`.

**Progress output (to stderr):**
```
Scanning 42 files...
```
After commit (to stdout):
```
Indexed 40 assets (2 skipped).
```

> **Note**: The deletion detection and confirmation prompt are implemented in Issue #14.
> This issue only covers the indexing path (new and changed files).

## Requirements

- [ ] Implement `scan` command handler in `cli` crate
- [ ] Use `walkdir` to enumerate `.uasset` and `.umap` files under `project_dir`
- [ ] Collect `(PathBuf, mtime_as_secs_u64)` from `DirEntry.metadata().modified()`
- [ ] Call `db.filter_changed()` to get changed/new paths (skipped when `--full-scan` is set)
- [ ] Pass filtered paths to `scanner::scan_files(&paths, &content_root)`
- [ ] Wrap `upsert_asset` + `replace_dependencies` calls in a `rusqlite::Transaction`
- [ ] Print `"Scanning N files..."` to stderr before the scan
- [ ] Print `"Indexed N assets (M skipped)."` to stdout after commit

## Related

- Depends on: #12 (CLI skeleton), #11 (asset-db read), #10 (asset-db write), #8 (scan_files)
- Next: #14 — deletion detection, output formatting, and exit codes
