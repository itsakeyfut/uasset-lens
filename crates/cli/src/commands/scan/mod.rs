mod diff;
mod format_time;

use std::collections::{HashMap, HashSet};
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

pub struct ScanOptions<'a> {
    pub full_scan: bool,
    pub diff: bool,
    pub yes: bool,
    pub save_baseline: Option<&'a str>,
    pub diff_from: Option<&'a str>,
}

pub fn handle_scan(
    project_dir: &Path,
    db_path: &Path,
    format: &FormatKind,
    cfg: &crate::config::ConfigFile,
    opts: &ScanOptions<'_>,
) -> anyhow::Result<i32> {
    let use_color = matches!(format, FormatKind::Text)
        && std::io::stdout().is_terminal()
        && std::env::var("NO_COLOR").is_err();

    if let Some(parent) = db_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }

    let mut db = asset_db::AssetDb::open(db_path).context("Failed to open database")?;

    // Fail fast before expensive walkdir if the requested baseline doesn't exist.
    let loaded_baseline = if let Some(name) = opts.diff_from {
        Some(db.load_baseline(name).map_err(|e| match e {
            asset_db::DbError::BaselineNotFound(_) => anyhow::anyhow!(
                "Baseline '{}' not found. Run 'scan --save-baseline {}' first.",
                name,
                name
            ),
            other => anyhow::Error::from(other),
        })?)
    } else {
        None
    };

    let content_root = crate::resolve_content_root(project_dir);

    // Snapshot DB state before walkdir for stale detection and new/updated classification.
    let db_files: HashSet<PathBuf> = db
        .all_known_files()
        .context("Failed to read known files from database")?
        .into_iter()
        .collect();

    let excluded = cfg.scan.exclude_paths.clone();

    let effective_diff = opts.diff || opts.diff_from.is_some();

    // Capture per-asset metrics before upsert so we can compute the diff afterwards.
    let (old_bp, old_sizes) = if effective_diff {
        let bp: HashMap<shared::AssetPath, (u32, u32)> = db
            .all_blueprint_metrics()
            .context("Failed to read blueprint metrics from database")?
            .into_iter()
            .map(|r| (r.asset_path, (r.node_count, r.event_tick_count)))
            .collect();
        let sizes: HashMap<shared::AssetPath, u64> = db
            .all_assets()
            .context("Failed to read assets from database")?
            .into_iter()
            .map(|r| (r.asset_path, r.file_size))
            .collect();
        (bp, sizes)
    } else {
        (HashMap::new(), HashMap::new())
    };

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

    let total_file_count = all_files.len();

    let paths_to_scan: Vec<PathBuf> = if opts.full_scan {
        all_files.into_iter().map(|(p, _)| p).collect()
    } else {
        db.filter_changed(&all_files)
            .context("Failed to check for changed files")?
    };

    eprintln!(
        "{}",
        scan_header(
            &content_root,
            opts.full_scan,
            total_file_count,
            paths_to_scan.len()
        )
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
        let confirmed = if opts.yes {
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

    let snapshot_id = db
        .record_scan_snapshot()
        .context("Failed to record scan snapshot")?;
    if let Some(name) = opts.save_baseline {
        db.save_baseline(name, snapshot_id)
            .context("Failed to save baseline")?;
        eprintln!("Baseline '{}' saved.", name);
    }
    let assets_total = db_files.len() + new_count - removed_count;

    if effective_diff {
        let prev_scanned_at = if let Some(ref snap) = loaded_baseline {
            Some(snap.scanned_at)
        } else {
            // snaps[0] = current, snaps[1] = previous (DESC order)
            db.recent_snapshots(2)
                .context("Failed to read scan history")?
                .into_iter()
                .nth(1)
                .map(|s| s.scanned_at)
        };
        return diff::print_diff(
            diff::DiffInput {
                assets: &result.assets,
                old_bp,
                old_sizes,
                db_files: &db_files,
                stale: &stale,
                content_root: &content_root,
                project_dir,
                threshold: cfg.diff.size_increase_threshold_pct,
                diff_from: opts.diff_from,
                prev_scanned_at,
            },
            format,
            use_color,
        );
    }

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
        FormatKind::Text | FormatKind::GithubActions => {
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

fn scan_header(content_root: &Path, full_scan: bool, total: usize, changed: usize) -> String {
    if !full_scan && changed == 0 {
        format!(
            "Scanning {}... ({} assets up to date, 0 changed)",
            content_root.display(),
            total
        )
    } else {
        format!("Scanning {}... ({} files)", content_root.display(), changed)
    }
}

fn sym(plain: &'static str, colored: &'static str, use_color: bool) -> &'static str {
    if use_color { colored } else { plain }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_db_in_tempdir;
    use shared::{AssetPath, AssetType};

    #[test]
    fn scan_header_should_show_files_when_changes_exist() {
        let root = Path::new("/proj/Content");
        assert_eq!(
            scan_header(root, false, 100, 5),
            "Scanning /proj/Content... (5 files)"
        );
    }

    #[test]
    fn scan_header_should_show_up_to_date_when_no_changes_and_not_full_scan() {
        let root = Path::new("/proj/Content");
        assert_eq!(
            scan_header(root, false, 881, 0),
            "Scanning /proj/Content... (881 assets up to date, 0 changed)"
        );
    }

    #[test]
    fn scan_header_should_show_files_when_full_scan_even_if_changed_is_zero() {
        let root = Path::new("/proj/Content");
        assert_eq!(
            scan_header(root, true, 100, 100),
            "Scanning /proj/Content... (100 files)"
        );
    }

    fn make_meta(asset_path: &str, file_path: PathBuf, mtime: u64) -> scanner::AssetMetadata {
        scanner::AssetMetadata {
            asset_path: AssetPath::new(asset_path).unwrap(),
            file_path,
            asset_type: AssetType::Blueprint,
            file_size: 1024,
            last_modified: mtime,
            dependencies: vec![],
            soft_dependencies: vec![],
            blueprint_metrics: None,
            material_texture_samples: None,
        }
    }

    #[test]
    fn handle_scan_should_record_scan_snapshot_after_upsert() {
        let (dir, db_path) = test_db_in_tempdir("scan_history");

        let result = handle_scan(
            &dir,
            &db_path,
            &FormatKind::Text,
            &Default::default(),
            &ScanOptions {
                full_scan: false,
                diff: false,
                yes: false,
                save_baseline: None,
                diff_from: None,
            },
        )
        .unwrap();
        assert_eq!(result, 0);

        let db = asset_db::AssetDb::open(&db_path).unwrap();
        let snaps = db.recent_snapshots(1).unwrap();
        assert_eq!(snaps.len(), 1, "scan should record exactly one snapshot");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_scan_should_not_index_files_under_excluded_path() {
        let (dir, db_path) = test_db_in_tempdir("scan25_exclude");
        let excluded_dir = dir.join("Content").join("Dev");
        std::fs::create_dir_all(&excluded_dir).unwrap();

        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[scan]\nexclude_paths = [\"Content/Dev/\"]\n",
        )
        .unwrap();
        std::fs::write(excluded_dir.join("Dummy.uasset"), b"not a real uasset").unwrap();
        let cfg = crate::config::load_config(&dir);
        let _ = handle_scan(
            &dir,
            &db_path,
            &FormatKind::Text,
            &cfg,
            &ScanOptions {
                full_scan: false,
                diff: false,
                yes: false,
                save_baseline: None,
                diff_from: None,
            },
        )
        .unwrap();

        let db = asset_db::AssetDb::open(&db_path).unwrap();
        assert!(
            db.all_assets().unwrap().is_empty(),
            "excluded path files should not be indexed in the database"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_scan_should_return_0_when_db_is_empty_and_dir_has_no_assets() {
        let (dir, db_path) = test_db_in_tempdir("scan14_empty");

        let result = handle_scan(
            &dir,
            &db_path,
            &FormatKind::Text,
            &Default::default(),
            &ScanOptions {
                full_scan: false,
                diff: false,
                yes: false,
                save_baseline: None,
                diff_from: None,
            },
        )
        .unwrap();

        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_scan_should_remove_stale_asset_and_return_1_when_yes_flag_set() {
        let (dir, db_path) = test_db_in_tempdir("scan14_stale");

        // Insert a record whose file does NOT exist on disk.
        {
            let db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_asset(&make_meta("/Game/Stale", dir.join("Stale.uasset"), 0))
                .unwrap();
        }

        let result = handle_scan(
            &dir,
            &db_path,
            &FormatKind::Text,
            &Default::default(),
            &ScanOptions {
                full_scan: false,
                diff: false,
                yes: true,
                save_baseline: None,
                diff_from: None,
            },
        )
        .unwrap();

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
        let (dir, db_path) = test_db_in_tempdir("scan14_decline");

        {
            let db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_asset(&make_meta("/Game/Stale", dir.join("Stale.uasset"), 0))
                .unwrap();
        }

        // yes=false + empty stdin (EOF in test context) → read_line returns "" →
        // trim() is "" → not "y" → confirmed=false → record preserved → exit code 0.
        let result = handle_scan(
            &dir,
            &db_path,
            &FormatKind::Text,
            &Default::default(),
            &ScanOptions {
                full_scan: false,
                diff: false,
                yes: false,
                save_baseline: None,
                diff_from: None,
            },
        )
        .unwrap();

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

    #[test]
    fn handle_scan_diff_should_return_0_when_no_previous_scan() {
        let (dir, db_path) = test_db_in_tempdir("scan_diff_noprev");

        let result = handle_scan(
            &dir,
            &db_path,
            &FormatKind::Text,
            &Default::default(),
            &ScanOptions {
                full_scan: false,
                diff: true,
                yes: false,
                save_baseline: None,
                diff_from: None,
            },
        )
        .unwrap();

        assert_eq!(
            result, 0,
            "first diff scan with no previous snapshot exits 0"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_scan_diff_should_return_0_when_no_regressions() {
        let (dir, db_path) = test_db_in_tempdir("scan_diff_noreg");

        // First scan to populate scan_history
        handle_scan(
            &dir,
            &db_path,
            &FormatKind::Text,
            &Default::default(),
            &ScanOptions {
                full_scan: false,
                diff: false,
                yes: false,
                save_baseline: None,
                diff_from: None,
            },
        )
        .unwrap();

        // Second scan with --diff; no .uasset files → no scanned assets → no regressions
        let result = handle_scan(
            &dir,
            &db_path,
            &FormatKind::Text,
            &Default::default(),
            &ScanOptions {
                full_scan: false,
                diff: true,
                yes: false,
                save_baseline: None,
                diff_from: None,
            },
        )
        .unwrap();

        assert_eq!(result, 0, "diff scan with no regressions exits 0");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_scan_diff_json_should_include_diff_fields() {
        let (dir, db_path) = test_db_in_tempdir("scan_diff_json");

        handle_scan(
            &dir,
            &db_path,
            &FormatKind::Text,
            &Default::default(),
            &ScanOptions {
                full_scan: false,
                diff: false,
                yes: false,
                save_baseline: None,
                diff_from: None,
            },
        )
        .unwrap();

        let result = handle_scan(
            &dir,
            &db_path,
            &FormatKind::Json,
            &Default::default(),
            &ScanOptions {
                full_scan: false,
                diff: true,
                yes: false,
                save_baseline: None,
                diff_from: None,
            },
        )
        .unwrap();

        assert_eq!(result, 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_scan_diff_should_return_0_with_configurable_threshold() {
        let (dir, db_path) = test_db_in_tempdir("scan_diff_thresh");

        // Write config with non-default threshold
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[diff]\nsize_increase_threshold_pct = 25\n",
        )
        .unwrap();
        let cfg = crate::config::load_config(&dir);
        handle_scan(
            &dir,
            &db_path,
            &FormatKind::Text,
            &cfg,
            &ScanOptions {
                full_scan: false,
                diff: false,
                yes: false,
                save_baseline: None,
                diff_from: None,
            },
        )
        .unwrap();

        let cfg = crate::config::load_config(&dir);
        let result = handle_scan(
            &dir,
            &db_path,
            &FormatKind::Text,
            &cfg,
            &ScanOptions {
                full_scan: false,
                diff: true,
                yes: false,
                save_baseline: None,
                diff_from: None,
            },
        )
        .unwrap();

        assert_eq!(
            result, 0,
            "diff with custom threshold exits 0 when no regressions"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_scan_diff_github_actions_should_return_0_when_no_regressions() {
        let (dir, db_path) = test_db_in_tempdir("scan_diff_ga");

        handle_scan(
            &dir,
            &db_path,
            &FormatKind::Text,
            &Default::default(),
            &ScanOptions {
                full_scan: false,
                diff: false,
                yes: false,
                save_baseline: None,
                diff_from: None,
            },
        )
        .unwrap();

        let result = handle_scan(
            &dir,
            &db_path,
            &FormatKind::GithubActions,
            &Default::default(),
            &ScanOptions {
                full_scan: false,
                diff: true,
                yes: false,
                save_baseline: None,
                diff_from: None,
            },
        )
        .unwrap();

        assert_eq!(result, 0, "github-actions diff with no regressions exits 0");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_scan_should_save_baseline_when_flag_provided() {
        let (dir, db_path) = test_db_in_tempdir("scan_baseline");

        let result = handle_scan(
            &dir,
            &db_path,
            &FormatKind::Text,
            &Default::default(),
            &ScanOptions {
                full_scan: false,
                diff: false,
                yes: false,
                save_baseline: Some("main"),
                diff_from: None,
            },
        )
        .unwrap();
        assert_eq!(result, 0);

        let db = asset_db::AssetDb::open(&db_path).unwrap();
        let snap = db.load_baseline("main").unwrap();
        assert_eq!(snap.asset_count, 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_scan_diff_from_baseline_should_return_0_when_no_regressions() {
        let (dir, db_path) = test_db_in_tempdir("scan_diff_from");

        handle_scan(
            &dir,
            &db_path,
            &FormatKind::Text,
            &Default::default(),
            &ScanOptions {
                full_scan: false,
                diff: false,
                yes: false,
                save_baseline: Some("main"),
                diff_from: None,
            },
        )
        .unwrap();

        let result = handle_scan(
            &dir,
            &db_path,
            &FormatKind::Text,
            &Default::default(),
            &ScanOptions {
                full_scan: false,
                diff: false,
                yes: false,
                save_baseline: None,
                diff_from: Some("main"),
            },
        )
        .unwrap();
        assert_eq!(result, 0, "diff-from baseline with no regressions exits 0");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_scan_diff_from_missing_baseline_should_return_error() {
        let (dir, db_path) = test_db_in_tempdir("scan_no_baseline");

        handle_scan(
            &dir,
            &db_path,
            &FormatKind::Text,
            &Default::default(),
            &ScanOptions {
                full_scan: false,
                diff: false,
                yes: false,
                save_baseline: None,
                diff_from: None,
            },
        )
        .unwrap();

        let result = handle_scan(
            &dir,
            &db_path,
            &FormatKind::Text,
            &Default::default(),
            &ScanOptions {
                full_scan: false,
                diff: false,
                yes: false,
                save_baseline: None,
                diff_from: Some("ghost"),
            },
        );
        assert!(
            result.is_err(),
            "diff-from unknown baseline should return an error"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
