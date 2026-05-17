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

#[derive(serde::Serialize)]
struct BpRegressionEntry {
    path: String,
    old_node_count: u32,
    new_node_count: u32,
    old_event_tick_count: u32,
    new_event_tick_count: u32,
}

#[derive(serde::Serialize)]
struct SizeIncreaseEntry {
    path: String,
    old_size: u64,
    new_size: u64,
    pct_increase: u64,
}

#[derive(serde::Serialize)]
struct ScanDiffOutput {
    prev_scanned_at: Option<u64>,
    new_assets: Vec<String>,
    deleted_assets: Vec<String>,
    regressions: Vec<BpRegressionEntry>,
    size_increases: Vec<SizeIncreaseEntry>,
}

pub fn handle_scan(
    project_dir: &Path,
    full_scan: bool,
    diff: bool,
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

    // Capture per-asset metrics before upsert so we can compute the diff afterwards.
    let (old_bp, old_sizes) = if diff {
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

    let paths_to_scan: Vec<PathBuf> = if full_scan {
        all_files.into_iter().map(|(p, _)| p).collect()
    } else {
        db.filter_changed(&all_files)
            .context("Failed to check for changed files")?
    };

    eprintln!(
        "{}",
        scan_header(
            &content_root,
            full_scan,
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

    db.record_scan_snapshot()
        .context("Failed to record scan snapshot")?;
    let assets_total = db_files.len() + new_count - removed_count;

    if diff {
        // snaps[0] = current, snaps[1] = previous (DESC order)
        let prev_scanned_at = db
            .recent_snapshots(2)
            .context("Failed to read scan history")?
            .into_iter()
            .nth(1)
            .map(|s| s.scanned_at);

        let threshold = config.diff.size_increase_threshold_pct;

        let regressions = compute_regressions(result.assets.iter(), &old_bp);
        let size_increases = compute_size_increases(result.assets.iter(), &old_sizes, threshold);

        let new_asset_paths: Vec<String> = result
            .assets
            .iter()
            .filter(|a| !db_files.contains(&a.file_path))
            .map(|a| a.asset_path.as_str().to_owned())
            .collect();

        let deleted_asset_paths: Vec<String> = stale
            .iter()
            .filter_map(|p| shared::AssetPath::from_fs_path(&content_root, p).ok())
            .map(|ap| ap.as_str().to_owned())
            .collect();

        let has_regressions = !regressions.is_empty();

        match format {
            FormatKind::Json => {
                let out = ScanDiffOutput {
                    prev_scanned_at,
                    new_assets: new_asset_paths,
                    deleted_assets: deleted_asset_paths,
                    regressions,
                    size_increases,
                };
                println!(
                    "{}",
                    serde_json::to_string_pretty(&out)
                        .context("Failed to serialize diff output to JSON")?
                );
            }
            FormatKind::Text => {
                println!();
                match prev_scanned_at {
                    Some(ts) => println!("Diff vs previous scan ({}):", format_utc(ts)),
                    None => println!("Diff: (no previous scan to compare against)"),
                }
                if !new_asset_paths.is_empty() {
                    println!("  + {} new asset(s):", new_asset_paths.len());
                    for p in &new_asset_paths {
                        println!("      {p}");
                    }
                }
                if !deleted_asset_paths.is_empty() {
                    println!("  - {} deleted asset(s):", deleted_asset_paths.len());
                    for p in &deleted_asset_paths {
                        println!("      {p}");
                    }
                }
                if !regressions.is_empty() {
                    println!(
                        "  {} {} blueprint(s) regressed (node count increased):",
                        sym("!", "\x1b[31m!\x1b[0m", use_color),
                        regressions.len()
                    );
                    for r in &regressions {
                        let node_delta = r.new_node_count - r.old_node_count;
                        let tick_delta =
                            r.new_event_tick_count as i64 - r.old_event_tick_count as i64;
                        let tick_suffix = if tick_delta > 0 {
                            format!("  [EventTick +{tick_delta}]")
                        } else {
                            String::new()
                        };
                        println!(
                            "      {}  {} → {} nodes  (+{node_delta}){tick_suffix}",
                            r.path, r.old_node_count, r.new_node_count
                        );
                    }
                }
                if !size_increases.is_empty() {
                    println!(
                        "  ^ {} asset(s) grew by ≥{}%:",
                        size_increases.len(),
                        threshold
                    );
                    for s in &size_increases {
                        println!(
                            "      {}  {} → {} (+{}%)",
                            s.path,
                            crate::format_size(s.old_size),
                            crate::format_size(s.new_size),
                            s.pct_increase
                        );
                    }
                }
            }
        }

        return if has_regressions { Ok(1) } else { Ok(0) };
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

fn compute_regressions<'a>(
    assets: impl Iterator<Item = &'a scanner::AssetMetadata>,
    old_bp: &HashMap<shared::AssetPath, (u32, u32)>,
) -> Vec<BpRegressionEntry> {
    let mut v: Vec<BpRegressionEntry> = assets
        .filter_map(|a| {
            let bm = a.blueprint_metrics.as_ref()?;
            let &(old_nodes, old_tick) = old_bp.get(&a.asset_path)?;
            if bm.node_count > old_nodes {
                Some(BpRegressionEntry {
                    path: a.asset_path.as_str().to_owned(),
                    old_node_count: old_nodes,
                    new_node_count: bm.node_count,
                    old_event_tick_count: old_tick,
                    new_event_tick_count: bm.event_tick_count,
                })
            } else {
                None
            }
        })
        .collect();
    v.sort_by(|a, b| a.path.cmp(&b.path));
    v
}

fn compute_size_increases<'a>(
    assets: impl Iterator<Item = &'a scanner::AssetMetadata>,
    old_sizes: &HashMap<shared::AssetPath, u64>,
    threshold: u64,
) -> Vec<SizeIncreaseEntry> {
    let mut v: Vec<SizeIncreaseEntry> = assets
        .filter_map(|a| {
            let &old_size = old_sizes.get(&a.asset_path)?;
            if old_size == 0 {
                return None;
            }
            let new_size = a.file_size;
            let pct = new_size.saturating_sub(old_size).saturating_mul(100) / old_size;
            if pct >= threshold {
                Some(SizeIncreaseEntry {
                    path: a.asset_path.as_str().to_owned(),
                    old_size,
                    new_size,
                    pct_increase: pct,
                })
            } else {
                None
            }
        })
        .collect();
    v.sort_by(|a, b| a.path.cmp(&b.path));
    v
}

fn format_utc(unix_secs: u64) -> String {
    let s = unix_secs % 60;
    let m = (unix_secs / 60) % 60;
    let h = (unix_secs / 3600) % 24;
    let mut rem_days = unix_secs / 86400;

    let mut year = 1970u64;
    loop {
        let dy = if is_leap_year(year) { 366u64 } else { 365u64 };
        if rem_days < dy {
            break;
        }
        rem_days -= dy;
        year += 1;
    }

    let month_lengths: [u64; 12] = [
        31,
        if is_leap_year(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u64;
    for &ml in &month_lengths {
        if rem_days < ml {
            break;
        }
        rem_days -= ml;
        month += 1;
    }
    let day = rem_days + 1;

    format!("{year:04}-{month:02}-{day:02} {h:02}:{m:02}:{s:02} UTC")
}

fn is_leap_year(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_scan_history_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        let result = handle_scan(&dir, false, false, &db_path, &FormatKind::Text, false).unwrap();
        assert_eq!(result, 0);

        let db = asset_db::AssetDb::open(&db_path).unwrap();
        let snaps = db.recent_snapshots(1).unwrap();
        assert_eq!(snaps.len(), 1, "scan should record exactly one snapshot");

        let _ = std::fs::remove_dir_all(&dir);
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
        let _ = handle_scan(&dir, false, false, &db_path, &FormatKind::Text, false).unwrap();

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

        let result = handle_scan(&dir, false, false, &db_path, &FormatKind::Text, false).unwrap();

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

        let result = handle_scan(&dir, false, false, &db_path, &FormatKind::Text, true).unwrap();

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
        let result = handle_scan(&dir, false, false, &db_path, &FormatKind::Text, false).unwrap();

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
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_scan_diff_noprev_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        let result = handle_scan(&dir, false, true, &db_path, &FormatKind::Text, false).unwrap();

        assert_eq!(
            result, 0,
            "first diff scan with no previous snapshot exits 0"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_scan_diff_should_return_0_when_no_regressions() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_scan_diff_noreg_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        // First scan to populate scan_history
        handle_scan(&dir, false, false, &db_path, &FormatKind::Text, false).unwrap();

        // Second scan with --diff; no .uasset files → no scanned assets → no regressions
        let result = handle_scan(&dir, false, true, &db_path, &FormatKind::Text, false).unwrap();

        assert_eq!(result, 0, "diff scan with no regressions exits 0");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_scan_diff_json_should_include_diff_fields() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_scan_diff_json_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        handle_scan(&dir, false, false, &db_path, &FormatKind::Text, false).unwrap();

        let result = handle_scan(&dir, false, true, &db_path, &FormatKind::Json, false).unwrap();

        assert_eq!(result, 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_scan_diff_should_return_0_with_configurable_threshold() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_scan_diff_thresh_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        // Write config with non-default threshold
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[diff]\nsize_increase_threshold_pct = 25\n",
        )
        .unwrap();

        let db_path = dir.join("test.db");
        handle_scan(&dir, false, false, &db_path, &FormatKind::Text, false).unwrap();

        let result = handle_scan(&dir, false, true, &db_path, &FormatKind::Text, false).unwrap();

        assert_eq!(
            result, 0,
            "diff with custom threshold exits 0 when no regressions"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn make_bp_meta(
        asset_path: &str,
        node_count: u32,
        event_tick_count: u32,
    ) -> scanner::AssetMetadata {
        let bm = scanner::BlueprintMetrics {
            node_count,
            event_tick_count,
            cast_count: 0,
            dependency_depth: 0,
        };
        scanner::AssetMetadata {
            file_path: PathBuf::from(format!("/proj/Content/{}.uasset", asset_path)),
            file_size: 1024,
            last_modified: 0,
            blueprint_metrics: Some(bm),
            ..scanner::make_meta(asset_path, AssetType::Blueprint)
        }
    }

    #[test]
    fn compute_regressions_should_detect_node_count_increase() {
        let mut old_bp = HashMap::new();
        old_bp.insert(AssetPath::new("/Game/BP_A").unwrap(), (10u32, 0u32));
        let meta = make_bp_meta("/Game/BP_A", 50, 2);

        let result = compute_regressions(std::iter::once(&meta), &old_bp);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].old_node_count, 10);
        assert_eq!(result[0].new_node_count, 50);
        assert_eq!(result[0].old_event_tick_count, 0);
        assert_eq!(result[0].new_event_tick_count, 2);
    }

    #[test]
    fn compute_regressions_should_not_flag_unchanged_node_count() {
        let mut old_bp = HashMap::new();
        old_bp.insert(AssetPath::new("/Game/BP_A").unwrap(), (10u32, 0u32));
        let meta = make_bp_meta("/Game/BP_A", 10, 0);

        let result = compute_regressions(std::iter::once(&meta), &old_bp);

        assert!(result.is_empty());
    }

    #[test]
    fn compute_regressions_should_not_flag_node_count_decrease() {
        let mut old_bp = HashMap::new();
        old_bp.insert(AssetPath::new("/Game/BP_A").unwrap(), (50u32, 0u32));
        let meta = make_bp_meta("/Game/BP_A", 10, 0);

        let result = compute_regressions(std::iter::once(&meta), &old_bp);

        assert!(result.is_empty());
    }

    #[test]
    fn compute_regressions_should_not_flag_new_asset_with_no_previous_record() {
        let old_bp: HashMap<AssetPath, (u32, u32)> = HashMap::new();
        let meta = make_bp_meta("/Game/BP_New", 42, 0);

        let result = compute_regressions(std::iter::once(&meta), &old_bp);

        assert!(result.is_empty());
    }

    #[test]
    fn compute_regressions_should_sort_results_by_path() {
        let mut old_bp = HashMap::new();
        old_bp.insert(AssetPath::new("/Game/Z_BP").unwrap(), (1u32, 0u32));
        old_bp.insert(AssetPath::new("/Game/A_BP").unwrap(), (1u32, 0u32));
        let meta_z = make_bp_meta("/Game/Z_BP", 99, 0);
        let meta_a = make_bp_meta("/Game/A_BP", 99, 0);

        let result = compute_regressions([&meta_z, &meta_a].into_iter(), &old_bp);

        assert_eq!(result.len(), 2);
        assert!(result[0].path < result[1].path);
    }

    #[test]
    fn compute_size_increases_should_detect_increase_above_threshold() {
        let mut old_sizes = HashMap::new();
        old_sizes.insert(AssetPath::new("/Game/T_Rock").unwrap(), 1000u64);
        let meta = scanner::AssetMetadata {
            file_path: PathBuf::from("/proj/Content/T_Rock.uasset"),
            file_size: 1200,
            last_modified: 0,
            ..scanner::make_meta("/Game/T_Rock", AssetType::Texture2D)
        };

        let result = compute_size_increases(std::iter::once(&meta), &old_sizes, 10);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].old_size, 1000);
        assert_eq!(result[0].new_size, 1200);
        assert_eq!(result[0].pct_increase, 20);
    }

    #[test]
    fn compute_size_increases_should_ignore_increase_below_threshold() {
        let mut old_sizes = HashMap::new();
        old_sizes.insert(AssetPath::new("/Game/T_Rock").unwrap(), 1000u64);
        let meta = scanner::AssetMetadata {
            file_path: PathBuf::from("/proj/Content/T_Rock.uasset"),
            file_size: 1050,
            last_modified: 0,
            ..scanner::make_meta("/Game/T_Rock", AssetType::Texture2D)
        };

        let result = compute_size_increases(std::iter::once(&meta), &old_sizes, 10);

        assert!(result.is_empty());
    }

    #[test]
    fn compute_size_increases_should_ignore_asset_with_no_previous_size() {
        let old_sizes: HashMap<AssetPath, u64> = HashMap::new();
        let meta = scanner::AssetMetadata {
            file_path: PathBuf::from("/proj/Content/T_New.uasset"),
            file_size: 5000,
            last_modified: 0,
            ..scanner::make_meta("/Game/T_New", AssetType::Texture2D)
        };

        let result = compute_size_increases(std::iter::once(&meta), &old_sizes, 10);

        assert!(result.is_empty());
    }

    #[test]
    fn format_utc_should_format_epoch_zero_as_unix_origin() {
        assert_eq!(format_utc(0), "1970-01-01 00:00:00 UTC");
    }

    #[test]
    fn format_utc_should_format_one_day_correctly() {
        assert_eq!(format_utc(86400), "1970-01-02 00:00:00 UTC");
    }

    #[test]
    fn format_utc_should_handle_leap_year_correctly() {
        // 1972 is a leap year; 1972-02-29 exists
        // days from 1970-01-01 to 1972-02-29:
        //   1970: 365, 1971: 365, then 31 (Jan) + 29 (Feb day 29) - 1 = 59 days into 1972
        //   total = 365 + 365 + 59 = 789 days → epoch 789 * 86400 = 68169600
        assert_eq!(format_utc(68_169_600), "1972-02-29 00:00:00 UTC");
    }
}
