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
pub mod redirectors;
pub mod scan;
pub mod stats;
pub mod watch;

#[cfg(test)]
pub(crate) fn make_meta(
    asset_path: &str,
    file_path: std::path::PathBuf,
    asset_type: shared::AssetType,
    file_size: u64,
    deps: Vec<shared::AssetPath>,
) -> scanner::AssetMetadata {
    scanner::AssetMetadata {
        asset_path: shared::AssetPath::new(asset_path).unwrap(),
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
