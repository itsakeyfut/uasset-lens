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
pub struct BlueprintRow {
    pub asset_path: AssetPath,
    pub asset_type: AssetType,
    pub node_count: u32,
    pub event_tick_count: u32,
    pub cast_count: u32,
    pub dependency_depth: u32,
}

#[derive(Debug)]
pub struct AssetFilter {
    pub asset_type: Option<AssetType>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub path_pattern: Option<String>,
}

#[derive(Debug)]
pub struct ScanSnapshot {
    pub id: i64,
    pub scanned_at: u64,
    pub asset_count: u64,
    pub total_size: u64,
    pub blueprint_count: u64,
    pub avg_node_count: f64,
    pub texture_count: u64,
    pub texture_size: u64,
}
