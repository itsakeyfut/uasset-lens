pub mod blueprint;
pub mod budget;
pub mod check;
pub mod clean;
pub mod dead_assets;
pub mod deps;
pub mod duplicates;
pub mod find;
pub mod graph;
pub mod impact;
pub mod lint;
pub mod path_conv;
pub mod redirectors;
pub mod scan;
pub mod stats;
pub mod watch;

#[cfg(test)]
pub(crate) fn make_meta(
    asset_path: &str,
    file_path: std::path::PathBuf,
    asset_type: uasset_lens_shared::AssetType,
    file_size: u64,
    deps: Vec<uasset_lens_shared::AssetPath>,
) -> uasset_lens_scanner::AssetMetadata {
    uasset_lens_scanner::AssetMetadata {
        asset_path: uasset_lens_shared::AssetPath::new(asset_path).unwrap(),
        file_path,
        asset_type,
        file_size,
        last_modified: 0,
        dependencies: deps,
        soft_dependencies: vec![],
        blueprint_metrics: None,
        material_texture_samples: None,
    }
}

#[cfg(test)]
pub(crate) fn test_db_in_tempdir(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("uasset_lens_{}_{}", tag, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("test.db");
    (dir, db_path)
}
