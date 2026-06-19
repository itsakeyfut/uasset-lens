#![allow(clippy::unwrap_used, clippy::expect_used)]
// Test-support helpers (compiled under #[cfg(test)] or the `test-support` feature) may unwrap.

use std::path::PathBuf;
use uasset_lens_shared::{AssetPath, AssetType};

use crate::db::AssetDb;
use crate::record::AssetRecord;

/// Test-only: forces the on-disk schema version, to simulate an outdated DB for `doctor` tests.
pub fn set_schema_version(db: &AssetDb, version: i64) {
    db.conn
        .pragma_update(None, "user_version", version)
        .unwrap();
}

pub fn make_record(asset_path: &str, asset_type: AssetType) -> AssetRecord {
    AssetRecord {
        id: 0,
        asset_path: AssetPath::new(asset_path).unwrap(),
        file_path: PathBuf::from(format!("{asset_path}.uasset")),
        asset_type,
        file_size: 0,
        last_modified: 0,
    }
}
