# `crates/asset-db` — SQLite schema, AssetRecord, AssetFilter, and `open()`

## Summary

Create the `asset-db` crate with the SQLite schema, the `AssetRecord` and `AssetFilter`
types, and the `AssetDb::open()` constructor.
Complete when `AssetDb::open(":memory:")` creates a fresh in-memory database with the
expected tables and indexes.

## Design Notes

**Schema:**

```sql
CREATE TABLE IF NOT EXISTS assets (
    id            INTEGER PRIMARY KEY,
    asset_path    TEXT    UNIQUE NOT NULL,
    file_path     TEXT    NOT NULL,
    asset_type    TEXT    NOT NULL,
    file_size     INTEGER NOT NULL,
    last_modified INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS dependencies (
    from_id  INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    to_path  TEXT    NOT NULL,
    PRIMARY KEY (from_id, to_path)
);

CREATE INDEX IF NOT EXISTS idx_assets_last_modified ON assets(last_modified);
CREATE INDEX IF NOT EXISTS idx_assets_asset_type    ON assets(asset_type);
CREATE INDEX IF NOT EXISTS idx_deps_to_path         ON dependencies(to_path);
```

**Types:**

```rust
pub struct AssetRecord {
    pub id: i64,
    pub asset_path: AssetPath,
    pub file_path: PathBuf,
    pub asset_type: AssetType,
    pub file_size: u64,
    pub last_modified: u64,
}

pub struct AssetFilter {
    pub asset_type:   Option<AssetType>,
    pub min_size:     Option<u64>,
    pub max_size:     Option<u64>,
    pub path_pattern: Option<String>,  // glob pattern
}
```

`rusqlite` with `features = ["bundled"]` is already declared in `[workspace.dependencies]`.
Use `rusqlite::Connection` internally; do not expose it in the public API.

## Requirements

- [ ] Define `AssetDb` struct wrapping `rusqlite::Connection`
- [ ] Implement `AssetDb::open(db_path: &Path) -> Result<AssetDb>` (creates file if not exists)
- [ ] Execute `CREATE TABLE IF NOT EXISTS` for `assets` on `open()`
- [ ] Execute `CREATE TABLE IF NOT EXISTS` for `dependencies` with `ON DELETE CASCADE` on `open()`
- [ ] Create all 3 indexes on `open()`
- [ ] Define `AssetRecord` struct
- [ ] Define `AssetFilter` struct
- [ ] Unit test: `open(":memory:")` succeeds and all 3 tables exist (query `sqlite_master`)

## Related

- Depends on: #3 (AssetPath, AssetType)
- Next: #10 — write path
- Docs: `docs/roadmap/phase1/ROADMAP.md` — Task 4
