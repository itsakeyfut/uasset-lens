pub mod error;
pub mod parser;

pub use error::ScanError;

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rayon::prelude::*;
use shared::{AssetPath, AssetType};

use parser::export::parse_export_table;
use parser::header::parse_header;
use parser::import::{parse_import_class_names, parse_import_table};
use parser::name_table::parse_name_table;

pub struct AssetMetadata {
    pub asset_path: AssetPath,
    pub file_path: PathBuf,
    pub asset_type: AssetType,
    pub file_size: u64,
    pub last_modified: u64,
    pub dependencies: Vec<AssetPath>,
}

pub struct ScanResult {
    pub assets: Vec<AssetMetadata>,
    pub skipped: Vec<SkippedFile>,
}

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

    let asset_type = if is_umap {
        AssetType::World
    } else {
        let cls_names =
            parse_import_class_names(&data, hdr.import_offset, hdr.import_count, &name_table)?;
        parse_export_table(
            &data,
            hdr.export_offset,
            hdr.export_count,
            hdr.depends_offset,
            &cls_names,
        )?
    };

    let dependencies = parse_import_table(&data, hdr.import_offset, hdr.import_count, &name_table)?;

    let asset_path = AssetPath::from_fs_path(content_root, file)?;

    Ok(AssetMetadata {
        asset_path,
        file_path: file.to_path_buf(),
        asset_type,
        file_size,
        last_modified,
        dependencies,
    })
}
