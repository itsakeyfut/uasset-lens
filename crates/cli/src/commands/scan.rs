use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::Context;
use walkdir::WalkDir;

pub fn handle_scan(project_dir: &Path, full_scan: bool, db_path: &Path) -> anyhow::Result<i32> {
    if let Some(parent) = db_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }

    let mut db = asset_db::AssetDb::open(db_path).context("Failed to open database")?;
    let content_root = crate::resolve_content_root(project_dir);

    let all_files: Vec<(PathBuf, u64)> = WalkDir::new(project_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("uasset") || ext.eq_ignore_ascii_case("umap"))
                .unwrap_or(false)
        })
        .map(|e| {
            let mtime = e
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            (e.path().to_path_buf(), mtime)
        })
        .collect();

    let paths_to_scan: Vec<PathBuf> = if full_scan {
        all_files.into_iter().map(|(p, _)| p).collect()
    } else {
        db.filter_changed(&all_files)
            .context("Failed to check for changed files")?
    };

    eprintln!("Scanning {} files...", paths_to_scan.len());

    let result = scanner::scan_files(&paths_to_scan, &content_root);

    db.upsert_all(&result.assets)
        .context("Failed to write assets to database")?;

    println!(
        "Indexed {} assets ({} skipped).",
        result.assets.len(),
        result.skipped.len()
    );

    Ok(0)
}
