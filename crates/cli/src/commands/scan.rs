use std::collections::HashSet;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::Context;
use walkdir::WalkDir;

use crate::FormatKind;

#[derive(serde::Serialize)]
struct ScanOutput {
    assets_total: usize,
    new: usize,
    updated: usize,
    removed: usize,
    skipped: Vec<SkippedEntry>,
}

#[derive(serde::Serialize)]
struct SkippedEntry {
    path: String,
    reason: String,
}

pub fn handle_scan(
    project_dir: &Path,
    full_scan: bool,
    db_path: &Path,
    format: &FormatKind,
    yes: bool,
) -> anyhow::Result<i32> {
    let use_color = matches!(format, FormatKind::Text)
        && std::io::stdout().is_terminal()
        && std::env::var("NO_COLOR").is_err();

    if let Some(parent) = db_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }

    let mut db = asset_db::AssetDb::open(db_path).context("Failed to open database")?;
    let content_root = crate::resolve_content_root(project_dir);

    // Snapshot DB state before walkdir for stale detection and new/updated classification.
    let db_files: HashSet<PathBuf> = db
        .all_known_files()
        .context("Failed to read known files from database")?
        .into_iter()
        .collect();

    let config = crate::config::load_config(project_dir);
    let excluded = config.scan.exclude_paths;

    let all_files: Vec<(PathBuf, u64)> = WalkDir::new(project_dir)
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir()
                && !excluded.is_empty()
                && let Ok(rel) = e.path().strip_prefix(project_dir)
            {
                // normalize to forward slashes for cross-platform comparison
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                if !rel_str.is_empty() && excluded.iter().any(|p| rel_str.starts_with(p.as_str())) {
                    return false;
                }
            }
            true
        })
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

    let walkdir_paths: HashSet<&PathBuf> = all_files.iter().map(|(p, _)| p).collect();

    let mut stale: Vec<PathBuf> = db_files
        .iter()
        .filter(|p| !walkdir_paths.contains(p))
        .cloned()
        .collect();
    stale.sort(); // deterministic output order

    let paths_to_scan: Vec<PathBuf> = if full_scan {
        all_files.into_iter().map(|(p, _)| p).collect()
    } else {
        db.filter_changed(&all_files)
            .context("Failed to check for changed files")?
    };

    eprintln!(
        "Scanning {}... ({} files)",
        content_root.display(),
        paths_to_scan.len()
    );

    let result = scanner::scan_files(&paths_to_scan, &content_root);

    let new_count = result
        .assets
        .iter()
        .filter(|a| !db_files.contains(&a.file_path))
        .count();
    let updated_count = result
        .assets
        .iter()
        .filter(|a| db_files.contains(&a.file_path))
        .count();

    db.upsert_all(&result.assets)
        .context("Failed to write assets to database")?;

    // Print per-category scan results before prompting for stale removal.
    if new_count > 0 {
        println!(
            "  {} {} new asset(s) indexed",
            sym("+", "\x1b[32m+\x1b[0m", use_color),
            new_count
        );
    }
    if updated_count > 0 {
        println!(
            "  {} {} asset(s) updated (mtime changed)",
            sym("~", "\x1b[33m~\x1b[0m", use_color),
            updated_count
        );
    }
    if !stale.is_empty() {
        println!(
            "  {} {} asset(s) removed from disk",
            sym("?", "\x1b[31m?\x1b[0m", use_color),
            stale.len()
        );
    }

    let removed_count = if stale.is_empty() {
        0
    } else {
        let confirmed = if yes {
            true
        } else {
            eprintln!();
            eprintln!("The following DB records have no corresponding file on disk:");
            for path in &stale {
                eprintln!("  {}", path.display());
            }
            eprint!("Remove {} record(s) from DB? [y/N]: ", stale.len());
            io::stderr().flush().ok();
            let mut answer = String::new();
            io::stdin()
                .read_line(&mut answer)
                .context("Failed to read from stdin")?;
            answer.trim().eq_ignore_ascii_case("y")
        };

        if confirmed {
            let mut removed = 0usize;
            for path in &stale {
                match shared::AssetPath::from_fs_path(&content_root, path) {
                    Ok(ap) => {
                        db.delete_asset(&ap)
                            .context("Failed to delete stale asset from database")?;
                        removed += 1;
                    }
                    Err(e) => {
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "Could not derive asset path for stale file; skipping deletion"
                        );
                    }
                }
            }
            removed
        } else {
            0
        }
    };

    let assets_total = db_files.len() + new_count - removed_count;

    match format {
        FormatKind::Json => {
            let skipped_entries: Vec<SkippedEntry> = result
                .skipped
                .iter()
                .map(|sf| SkippedEntry {
                    path: sf.file_path.to_string_lossy().into_owned(),
                    reason: sf.reason.to_string(),
                })
                .collect();
            let output = ScanOutput {
                assets_total,
                new: new_count,
                updated: updated_count,
                removed: removed_count,
                skipped: skipped_entries,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&output)
                    .context("Failed to serialize scan output to JSON")?
            );
        }
        FormatKind::Text => {
            println!();
            println!(
                "  {} {} assets total, {} record(s) cleaned, {} skipped (parse error)",
                sym("✓", "\x1b[32m✓\x1b[0m", use_color),
                assets_total,
                removed_count,
                result.skipped.len()
            );

            if !result.skipped.is_empty() {
                eprintln!();
                eprintln!("Skipped:");
                for sf in &result.skipped {
                    eprintln!("  WARN {}: {}", sf.file_path.display(), sf.reason);
                }
            }
        }
    }

    if removed_count > 0 { Ok(1) } else { Ok(0) }
}

fn sym(plain: &'static str, colored: &'static str, use_color: bool) -> &'static str {
    if use_color { colored } else { plain }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::{AssetPath, AssetType};

    fn make_meta(asset_path: &str, file_path: PathBuf, mtime: u64) -> scanner::AssetMetadata {
        scanner::AssetMetadata {
            asset_path: AssetPath::new(asset_path).unwrap(),
            file_path,
            asset_type: AssetType::Blueprint,
            file_size: 1024,
            last_modified: mtime,
            dependencies: vec![],
            blueprint_metrics: None,
        }
    }

    #[test]
    fn handle_scan_should_not_index_files_under_excluded_path() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_scan25_exclude_{}", std::process::id()));
        let excluded_dir = dir.join("Content").join("Dev");
        std::fs::create_dir_all(&excluded_dir).unwrap();

        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[scan]\nexclude_paths = [\"Content/Dev/\"]\n",
        )
        .unwrap();
        std::fs::write(excluded_dir.join("Dummy.uasset"), b"not a real uasset").unwrap();

        let db_path = dir.join("test.db");
        let _ = handle_scan(&dir, false, &db_path, &FormatKind::Text, false).unwrap();

        let db = asset_db::AssetDb::open(&db_path).unwrap();
        assert!(
            db.all_assets().unwrap().is_empty(),
            "excluded path files should not be indexed in the database"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_scan_should_return_0_when_db_is_empty_and_dir_has_no_assets() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_scan14_empty_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        let result = handle_scan(&dir, false, &db_path, &FormatKind::Text, false).unwrap();

        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_scan_should_remove_stale_asset_and_return_1_when_yes_flag_set() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_scan14_stale_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        // Insert a record whose file does NOT exist on disk.
        {
            let db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_asset(&make_meta("/Game/Stale", dir.join("Stale.uasset"), 0))
                .unwrap();
        }

        let result = handle_scan(&dir, false, &db_path, &FormatKind::Text, true).unwrap();

        assert_eq!(
            result, 1,
            "exit code should be 1 when stale assets are removed"
        );

        let db = asset_db::AssetDb::open(&db_path).unwrap();
        assert!(
            db.all_assets().unwrap().is_empty(),
            "stale asset should have been removed from the database"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_scan_should_preserve_record_when_stdin_empty_and_no_yes_flag() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_scan14_decline_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_asset(&make_meta("/Game/Stale", dir.join("Stale.uasset"), 0))
                .unwrap();
        }

        // yes=false + empty stdin (EOF in test context) → read_line returns "" →
        // trim() is "" → not "y" → confirmed=false → record preserved → exit code 0.
        let result = handle_scan(&dir, false, &db_path, &FormatKind::Text, false).unwrap();

        assert_eq!(
            result, 0,
            "exit code should be 0 when removal is not confirmed"
        );

        let db = asset_db::AssetDb::open(&db_path).unwrap();
        assert_eq!(
            db.all_assets().unwrap().len(),
            1,
            "stale record should be preserved when removal is not confirmed"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
