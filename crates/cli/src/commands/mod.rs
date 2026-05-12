pub mod dead_assets;
pub mod find;
pub mod graph;
pub mod impact;
pub mod redirectors;
pub mod scan;

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
        blueprint_metrics: None,
        material_texture_samples: None,
    }
}
