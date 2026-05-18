mod anim_montage;
mod blueprint;
mod data_table;
pub mod error;
mod material;
pub mod parser;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
#[cfg(any(test, feature = "test-support"))]
pub use test_support::make_meta;

pub use blueprint::BlueprintMetrics;
pub use error::ScanError;
pub use parser::properties::{ParsedProperty, parse_properties};

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rayon::prelude::*;
use shared::{AssetPath, AssetType};

use parser::export::parse_export_table;
use parser::header::parse_header;
use parser::import::parse_import_entries;
use parser::name_table::parse_name_table;
use parser::soft_object_paths::parse_soft_object_paths;

#[derive(Debug)]
pub struct AssetMetadata {
    pub asset_path: AssetPath,
    pub file_path: PathBuf,
    pub asset_type: AssetType,
    pub file_size: u64,
    pub last_modified: u64,
    pub dependencies: Vec<AssetPath>,
    pub soft_dependencies: Vec<AssetPath>,
    pub blueprint_metrics: Option<BlueprintMetrics>,
    pub material_texture_samples: Option<u32>,
}

#[derive(Debug)]
pub struct ScanResult {
    pub assets: Vec<AssetMetadata>,
    pub skipped: Vec<SkippedFile>,
}

#[derive(Debug)]
pub struct SkippedFile {
    pub file_path: PathBuf,
    pub reason: ScanError,
}

pub fn scan_files(files: &[PathBuf], content_root: &Path) -> ScanResult {
    let pairs: Vec<Result<AssetMetadata, SkippedFile>> = files
        .par_iter()
        .map(|path| {
            scan_single(path, content_root).map_err(|reason| {
                tracing::warn!(path = %path.display(), reason = %reason, "Skipping file");
                SkippedFile {
                    // clone required: SkippedFile owns the path; par_iter yields references
                    file_path: path.clone(),
                    reason,
                }
            })
        })
        .collect();

    let mut assets = Vec::new();
    let mut skipped = Vec::new();
    for pair in pairs {
        match pair {
            Ok(meta) => assets.push(meta),
            Err(sf) => skipped.push(sf),
        }
    }
    ScanResult { assets, skipped }
}

fn scan_single(file: &Path, content_root: &Path) -> Result<AssetMetadata, ScanError> {
    let fs_meta = std::fs::metadata(file)?;
    let file_size = fs_meta.len();
    let last_modified = fs_meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let data = std::fs::read(file)?;

    let is_umap = file
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("umap"))
        .unwrap_or(false);

    let hdr = parse_header(&data)?;
    let name_table = parse_name_table(&data, hdr.name_offset, hdr.name_count)?;

    let (cls_names, dependencies) =
        parse_import_entries(&data, hdr.import_offset, hdr.import_count, &name_table)?;

    let mut soft_dependencies = parse_soft_object_paths(
        &data,
        hdr.soft_object_path_offset,
        hdr.soft_object_path_count,
        &name_table,
    )?;

    let asset_type = if is_umap {
        AssetType::World
    } else {
        parse_export_table(
            &data,
            hdr.export_offset,
            hdr.export_count,
            hdr.depends_offset,
            &cls_names,
        )?
    };

    let blueprint_metrics = if blueprint::is_blueprint_asset(&asset_type) {
        Some(blueprint::extract_blueprint_metrics(
            &data,
            hdr.export_offset,
            hdr.export_count,
            hdr.depends_offset,
            &cls_names,
            &dependencies,
            &name_table,
        ))
    } else {
        None
    };

    let material_texture_samples = if material::is_material_asset(&asset_type) {
        Some(material::extract_texture_sample_count(
            &data,
            hdr.export_offset,
            hdr.export_count,
            hdr.depends_offset,
            &cls_names,
        ))
    } else {
        None
    };

    let dt_soft_refs = if data_table::is_data_table_asset(&asset_type) {
        data_table::extract_data_table_soft_refs(
            &data,
            hdr.export_offset,
            hdr.export_count,
            hdr.depends_offset,
            &cls_names,
            &name_table,
        )
    } else {
        Vec::new()
    };

    soft_dependencies.extend(dt_soft_refs);

    let am_soft_refs = if anim_montage::is_anim_montage_asset(&asset_type) {
        anim_montage::extract_anim_montage_soft_refs(
            &data,
            hdr.export_offset,
            hdr.export_count,
            hdr.depends_offset,
            &cls_names,
            &name_table,
        )
    } else {
        Vec::new()
    };

    soft_dependencies.extend(am_soft_refs);

    let asset_path = AssetPath::from_fs_path(content_root, file)?;

    Ok(AssetMetadata {
        asset_path,
        file_path: file.to_path_buf(),
        asset_type,
        file_size,
        last_modified,
        dependencies,
        soft_dependencies,
        blueprint_metrics,
        material_texture_samples,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures");

    #[test]
    fn scan_files_should_set_blueprint_metrics_for_blueprint_asset() {
        let fixture = PathBuf::from(format!("{FIXTURES_DIR}/valid/BP_Simple.uasset"));
        let content_root = PathBuf::from(format!("{FIXTURES_DIR}/valid"));
        let result = scan_files(&[fixture], &content_root);

        assert!(result.skipped.is_empty());
        assert_eq!(result.assets.len(), 1);
        let meta = &result.assets[0];
        assert_eq!(meta.asset_type, AssetType::Blueprint);

        let metrics = meta
            .blueprint_metrics
            .as_ref()
            .expect("Blueprint should have metrics");
        assert!(
            metrics.node_count > 0,
            "Blueprint fixture must have at least one K2Node export"
        );
    }

    #[test]
    fn scan_files_should_set_material_texture_samples_for_material_asset() {
        let fixture = PathBuf::from(format!("{FIXTURES_DIR}/valid/M_Basic.uasset"));
        let content_root = PathBuf::from(format!("{FIXTURES_DIR}/valid"));
        let result = scan_files(&[fixture], &content_root);

        assert!(result.skipped.is_empty());
        assert_eq!(result.assets.len(), 1);
        let meta = &result.assets[0];
        assert_eq!(meta.asset_type, AssetType::Material);
        let count = meta
            .material_texture_samples
            .expect("Material should have material_texture_samples set");
        assert!(
            count > 0,
            "M_Basic fixture must have at least one MaterialExpressionTextureSample export"
        );
    }

    #[test]
    fn scan_files_should_set_no_blueprint_metrics_for_texture_asset() {
        let fixture = PathBuf::from(format!("{FIXTURES_DIR}/valid/T_Rock.uasset"));
        let content_root = PathBuf::from(format!("{FIXTURES_DIR}/valid"));
        let result = scan_files(&[fixture], &content_root);

        assert!(result.skipped.is_empty());
        assert_eq!(result.assets.len(), 1);
        let meta = &result.assets[0];
        assert!(
            meta.blueprint_metrics.is_none(),
            "Texture2D should not have blueprint metrics"
        );
        assert!(
            meta.material_texture_samples.is_none(),
            "Texture2D should not have material texture samples"
        );
    }
}
