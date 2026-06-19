#![allow(clippy::unwrap_used, clippy::expect_used)]
// Test-support helpers (compiled under #[cfg(test)] or the `test-support` feature) may unwrap.

use std::path::PathBuf;
use uasset_lens_shared::{AssetPath, AssetType};

use crate::AssetMetadata;

pub fn make_meta(asset_path: &str, asset_type: AssetType) -> AssetMetadata {
    AssetMetadata {
        asset_path: AssetPath::new(asset_path).unwrap(),
        file_path: PathBuf::from(format!("{asset_path}.uasset")),
        asset_type,
        file_size: 0,
        last_modified: 0,
        dependencies: vec![],
        soft_dependencies: vec![],
        blueprint_metrics: None,
        material_texture_samples: None,
    }
}
