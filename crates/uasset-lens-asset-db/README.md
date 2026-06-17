# uasset-lens-asset-db

> SQLite-backed index of Unreal Engine 5 assets, with duplicate detection.

[![crates.io](https://img.shields.io/crates/v/uasset-lens-asset-db.svg)](https://crates.io/crates/uasset-lens-asset-db)
[![docs.rs](https://docs.rs/uasset-lens-asset-db/badge.svg)](https://docs.rs/uasset-lens-asset-db)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Persists scanned `AssetMetadata` into a local SQLite database (via `rusqlite`) and exposes
typed queries (`AssetFilter`, `AssetRecord`) plus same-name / same-size duplicate detection.
Designed to back incremental mtime-delta scans of projects up to ~100,000 assets.

**Part of [uasset-lens](https://github.com/itsakeyfut/uasset-lens)** — a fast, CLI-first
static analyzer for Unreal Engine 5 assets that runs without opening the editor. This crate
is the on-disk asset index. Depends on: `uasset-lens-shared`, `uasset-lens-scanner`.
Used by: `uasset-lens-analysis`.

## Usage

```rust,no_run
use std::path::Path;
use uasset_lens_asset_db::AssetDb;
use uasset_lens_scanner::scan_files;

let content_root = Path::new("MyProject/Content");
let scan = scan_files(&[], content_root); // pass the project's .uasset paths here

let mut db = AssetDb::open(Path::new("assets.db"))?;
db.upsert_all(&scan.assets)?;
# Ok::<(), uasset_lens_asset_db::DbError>(())
```

## Feature flags

- `test-support`: exposes `make_record` and related fixtures used by downstream crate tests.

## Minimum supported Rust version

1.96.0

## License

Licensed under either of
[MIT](https://github.com/itsakeyfut/uasset-lens/blob/main/LICENSE-MIT) or
[Apache-2.0](https://github.com/itsakeyfut/uasset-lens/blob/main/LICENSE-APACHE)
at your option.
