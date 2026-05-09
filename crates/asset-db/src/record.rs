use std::path::PathBuf;

use shared::{AssetPath, AssetType};

#[derive(Debug)]
pub struct AssetRecord {
    pub id: i64,
    pub asset_path: AssetPath,
    pub file_path: PathBuf,
    pub asset_type: AssetType,
    pub file_size: u64,
    pub last_modified: u64,
}

#[derive(Debug)]
pub struct AssetFilter {
    pub asset_type: Option<AssetType>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub path_pattern: Option<String>,
}
