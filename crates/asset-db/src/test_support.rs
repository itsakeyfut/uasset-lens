use shared::{AssetPath, AssetType};
use std::path::PathBuf;

use crate::record::AssetRecord;

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
