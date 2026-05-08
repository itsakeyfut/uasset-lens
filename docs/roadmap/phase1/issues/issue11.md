# `crates/asset-db` — read-path query methods

## Summary

Implement the read-side query API on `AssetDb`: graph-loading methods for Phase 2
and the filtered asset search for Phase 3.
Complete when `find_assets()` returns correctly filtered results for all filter combinations.

## Design Notes

**Methods:**

- `all_assets()` — `SELECT * FROM assets` → `Vec<AssetRecord>` (used by Phase 2 to build `DependencyGraph`)
- `all_edges()` — `SELECT a.asset_path, d.to_path FROM dependencies d JOIN assets a ON d.from_id = a.id` → `Vec<(AssetPath, AssetPath)>`
- `get_asset(asset_path)` — `SELECT * FROM assets WHERE asset_path = ?` → `Option<AssetRecord>`
- `find_assets(filter)` — apply `asset_type` and size bounds in SQL `WHERE`, then filter `path_pattern` in Rust using `globset`

**`find_assets()` implementation strategy:**

Build the SQL `WHERE` clause dynamically based on which `AssetFilter` fields are `Some`.
After fetching rows, if `path_pattern` is set, compile a `globset::Glob` and retain only matching `file_path` values.
Using Rust-side glob filtering keeps the SQL query simple and avoids `LIKE` limitations.

Add `globset` to `[workspace.dependencies]`.

## Requirements

- [ ] Implement `all_assets() -> Result<Vec<AssetRecord>>`
- [ ] Implement `all_edges() -> Result<Vec<(AssetPath, AssetPath)>>`
- [ ] Implement `get_asset(asset_path: &AssetPath) -> Result<Option<AssetRecord>>`
- [ ] Implement `find_assets(filter: &AssetFilter) -> Result<Vec<AssetRecord>>`
- [ ] Add `globset` to `[workspace.dependencies]` and as a dependency of `asset-db`
- [ ] Unit test: `find_assets` with `asset_type` filter returns only matching rows
- [ ] Unit test: `find_assets` with `min_size` / `max_size` bounds
- [ ] Unit test: `find_assets` with `path_pattern` glob (e.g. `"**/Characters/**"`)
- [ ] Unit test: `find_assets` with combined filters
- [ ] Unit test: `find_assets` with no matching rows returns empty `Vec`

## Related

- Depends on: #10 (write path — need data to query)
- Used by: Phase 2 Issue #1 (dependency-graph build via `all_assets` + `all_edges`)
- Docs: `docs/roadmap/phase1/ROADMAP.md` — Task 4
