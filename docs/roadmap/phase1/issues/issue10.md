# `crates/asset-db` — write path and differential scan helpers

## Summary

Implement the write-side API on `AssetDb`: asset upsert/delete, dependency replacement,
and the differential-scan helpers used by the `scan` CLI command.
Complete when `filter_changed()` correctly returns only new and modified files.

## Design Notes

**Method contracts:**

- `upsert_asset(meta)` — `INSERT OR REPLACE INTO assets` using all `AssetMetadata` fields; returns the `rowid` (used by `replace_dependencies`)
- `delete_asset(asset_path)` — `DELETE FROM assets WHERE asset_path = ?`; the `ON DELETE CASCADE` on `dependencies` removes edges automatically
- `replace_dependencies(from_id, to_paths)` — `DELETE FROM dependencies WHERE from_id = ?`, then `INSERT INTO dependencies` for each path
- `filter_changed(files: &[(PathBuf, u64)])` — for each `(path, mtime)` pair, check if `assets` has a row with the same `file_path` and `last_modified == mtime`; return the paths that are missing or have a different mtime
- `all_known_files()` — `SELECT file_path FROM assets`; used by the CLI to detect stale assets

**Transaction ownership:** These methods do not manage transactions. The caller (CLI scan command) wraps bulk writes in a `rusqlite::Transaction` for performance.

## Requirements

- [ ] Implement `upsert_asset(meta: &AssetMetadata) -> Result<i64>`
- [ ] Implement `delete_asset(asset_path: &AssetPath) -> Result<()>`
- [ ] Implement `replace_dependencies(from_id: i64, to_paths: &[AssetPath]) -> Result<()>`
- [ ] Implement `filter_changed(files: &[(PathBuf, u64)]) -> Result<Vec<PathBuf>>`
- [ ] Implement `all_known_files() -> Result<Vec<PathBuf>>`
- [ ] Unit test: `filter_changed` — new file (not in DB) is returned
- [ ] Unit test: `filter_changed` — file with changed mtime is returned
- [ ] Unit test: `filter_changed` — file with unchanged mtime is NOT returned
- [ ] Unit test: `upsert_asset` followed by `delete_asset` → asset no longer in DB, dependencies cascade-deleted

## Related

- Depends on: #9 (schema + open())
- Next: #11 — read path
