#![doc = include_str!("../README.md")]

mod anim_montage;
mod blueprint;
mod data_table;
pub mod error;
mod level_sequence;
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
use uasset_lens_shared::{AssetPath, AssetType};

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

/// A file to scan plus the modification time captured when it was discovered. Threading the
/// directory walk's mtime through here lets the scanner avoid re-`stat`ing each file for `mtime`.
#[derive(Debug, Clone)]
pub struct ScanInput {
    pub path: PathBuf,
    pub mtime: u64,
}

pub fn scan_files(files: &[PathBuf], content_root: &Path) -> ScanResult {
    // Convenience for callers that have not already captured mtimes (watcher, git-diff, tests):
    // stat each file once here to build the `ScanInput`, then scan without a second stat.
    let inputs: Vec<ScanInput> = files
        .iter()
        .map(|p| ScanInput {
            path: p.clone(),
            mtime: file_mtime(p),
        })
        .collect();
    scan_files_with_progress(&inputs, content_root, || {})
}

/// Like [`scan_files`], but invokes `on_file` once as each file finishes parsing. The callback
/// runs from `rayon` worker threads (hence `Sync`); the CLI uses it to advance a progress bar.
pub fn scan_files_with_progress<F: Fn() + Sync>(
    inputs: &[ScanInput],
    content_root: &Path,
    on_file: F,
) -> ScanResult {
    // Canonicalize the content root once: the root never changes during a scan, so resolving it
    // per file inside par_iter would repeat one realpath/stat syscall for every asset (notably
    // slow on Windows). `None` (root canonicalize failed) falls back to per-file resolution below,
    // reproducing the prior all-files-skipped behavior for an unresolvable root.
    let canonical_root = content_root.canonicalize().ok();
    let pairs: Vec<Result<AssetMetadata, SkippedFile>> = inputs
        .par_iter()
        .map(|input| {
            let result =
                scan_single(input, content_root, canonical_root.as_deref()).map_err(|reason| {
                    tracing::warn!(path = %input.path.display(), reason = %reason, "Skipping file");
                    SkippedFile {
                        // clone required: SkippedFile owns the path; par_iter yields references
                        file_path: input.path.clone(),
                        reason,
                    }
                });
            on_file();
            result
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

fn scan_single(
    input: &ScanInput,
    content_root: &Path,
    canonical_root: Option<&Path>,
) -> Result<AssetMetadata, ScanError> {
    let data = std::fs::read(&input.path)?;
    // Size comes from the bytes just read and mtime from the caller-supplied `ScanInput`, so the
    // file is not re-`stat`ed here (the directory walk already captured its mtime).
    let file_size = data.len() as u64;
    let last_modified = input.mtime;

    let is_umap = input
        .path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("umap"))
        .unwrap_or(false);

    let hdr = parse_header(&data)?;
    let name_table = parse_name_table(&data, hdr.name_offset, hdr.name_count)?;

    let (cls_name_idxs, mut dependencies) =
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
            &cls_name_idxs,
            &name_table,
        )?
    };

    let blueprint_metrics = if blueprint::is_blueprint_asset(&asset_type) {
        Some(blueprint::extract_blueprint_metrics(
            &data,
            hdr.export_offset,
            hdr.export_count,
            hdr.depends_offset,
            &cls_name_idxs,
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
            &cls_name_idxs,
            &name_table,
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
            &cls_name_idxs,
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
            &cls_name_idxs,
            &name_table,
        )
    } else {
        Vec::new()
    };

    soft_dependencies.extend(am_soft_refs);

    let ls_soft_refs = if level_sequence::is_level_sequence_asset(&asset_type) {
        level_sequence::extract_level_sequence_soft_refs(
            &data,
            hdr.export_offset,
            hdr.export_count,
            hdr.depends_offset,
            &name_table,
        )
    } else {
        Vec::new()
    };

    soft_dependencies.extend(ls_soft_refs);

    // Deduplicate: import table can reference the same package multiple times, and multiple
    // soft-ref sources (DataTable, AnimMontage, LevelSequence) can overlap each other or the
    // hard dep list. Hard deps take priority, so soft deps that duplicate a hard dep are dropped.
    dependencies.sort_unstable_by(|a, b| a.as_str().cmp(b.as_str()));
    dependencies.dedup_by(|a, b| a.as_str() == b.as_str());
    {
        use std::collections::HashSet;
        let hard: HashSet<&str> = dependencies.iter().map(|d| d.as_str()).collect();
        soft_dependencies.retain(|d| !hard.contains(d.as_str()));
    }
    soft_dependencies.sort_unstable_by(|a, b| a.as_str().cmp(b.as_str()));
    soft_dependencies.dedup_by(|a, b| a.as_str() == b.as_str());

    let asset_path = match canonical_root {
        Some(root) => AssetPath::from_fs_path_with_canonical_root(root, &input.path)?,
        // Root canonicalize failed: fall back to per-file resolution (reproduces prior behavior).
        None => AssetPath::from_fs_path(content_root, &input.path)?,
    };

    Ok(AssetMetadata {
        asset_path,
        file_path: input.path.clone(),
        asset_type,
        file_size,
        last_modified,
        dependencies,
        soft_dependencies,
        blueprint_metrics,
        material_texture_samples,
    })
}

/// Reads a file's modification time as seconds since the Unix epoch, or 0 if it cannot be read.
fn file_mtime(path: &Path) -> u64 {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures");

    #[test]
    fn scan_files_with_progress_should_invoke_callback_for_each_file_including_skipped() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Root is the fixtures dir so both the valid and the invalid file resolve under it.
        let content_root = PathBuf::from(FIXTURES_DIR);
        let inputs = vec![
            ScanInput {
                path: PathBuf::from(format!("{FIXTURES_DIR}/valid/BP_Simple.uasset")),
                mtime: 0,
            },
            ScanInput {
                path: PathBuf::from(format!("{FIXTURES_DIR}/invalid/bad_magic.bin")),
                mtime: 0,
            },
        ];
        let calls = AtomicUsize::new(0);
        let result = scan_files_with_progress(&inputs, &content_root, || {
            calls.fetch_add(1, Ordering::Relaxed);
        });
        // The callback must fire for the skipped file too, so a progress bar reaches its total.
        assert_eq!(calls.load(Ordering::Relaxed), inputs.len());
        assert_eq!(result.assets.len(), 1);
        assert_eq!(result.skipped.len(), 1);
    }

    #[test]
    fn scan_files_with_progress_should_use_provided_mtime_without_restat() {
        // The scanner must trust the caller-supplied mtime instead of re-stat'ing the file: pass a
        // sentinel mtime and assert it survives to the AssetMetadata (a re-stat would overwrite it
        // with the file's real mtime). file_size is derived from the bytes read.
        let path = PathBuf::from(format!("{FIXTURES_DIR}/valid/BP_Simple.uasset"));
        let content_root = PathBuf::from(format!("{FIXTURES_DIR}/valid"));
        let expected_size = std::fs::read(&path).unwrap().len() as u64;
        let inputs = vec![ScanInput {
            path,
            mtime: 12_345,
        }];
        let result = scan_files_with_progress(&inputs, &content_root, || {});
        assert_eq!(result.assets.len(), 1);
        assert_eq!(result.assets[0].last_modified, 12_345);
        assert_eq!(result.assets[0].file_size, expected_size);
    }

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

    #[test]
    fn scan_files_should_extract_soft_refs_from_level_sequence_asset() {
        let fixture = PathBuf::from(format!("{FIXTURES_DIR}/valid/LS_Simple.uasset"));
        let content_root = PathBuf::from(format!("{FIXTURES_DIR}/valid"));
        let result = scan_files(&[fixture], &content_root);

        assert!(result.skipped.is_empty());
        assert_eq!(result.assets.len(), 1);
        let meta = &result.assets[0];
        assert_eq!(meta.asset_type, AssetType::LevelSequence);

        let paths: Vec<&str> = meta.soft_dependencies.iter().map(|p| p.as_str()).collect();
        assert!(
            paths.contains(&"/Game/Anims/AS_Run"),
            "expected /Game/Anims/AS_Run in soft_dependencies, got: {paths:?}"
        );
        assert!(
            paths.contains(&"/Game/Sounds/SW_Fire"),
            "expected /Game/Sounds/SW_Fire in soft_dependencies, got: {paths:?}"
        );
    }
}
