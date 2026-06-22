mod diff;

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
    /// Global `--quiet`: suppress the progress bar, the per-category breakdown, and
    /// informational stderr lines. The final summary line / JSON result on stdout is still
    /// printed (use `suppress_report` to silence that too).
    pub quiet: bool,
    /// Suppress scan's own stdout result (the summary line and the JSON object) entirely.
    /// Set by `check`'s auto-scan to keep its stdout clean; distinct from `quiet`, which keeps
    /// the result.
    pub suppress_report: bool,
    /// Global `--no-color`: force-disable ANSI color in the scan report.
    pub no_color: bool,
    /// Suppress the animated progress bar (set by `--no-progress`).
    pub no_progress: bool,
}

/// Whether to draw the animated scan progress bar. The bar goes to stderr, so it is shown only
/// for interactive text-format runs; it is suppressed for JSON output, `--quiet`, `--no-progress`,
/// and non-TTY stderr (pipes, redirects, CI).
fn should_show_progress(
    format: &FormatKind,
    quiet: bool,
    no_progress: bool,
    stderr_tty: bool,
) -> bool {
    matches!(format, FormatKind::Text) && !quiet && !no_progress && stderr_tty
}

/// True if a pattern uses glob metacharacters and must be matched as a glob (not a prefix).
fn is_glob_pattern(p: &str) -> bool {
    p.contains('*') || p.contains('?')
}

/// Normalizes the prefix subset of `scan.exclude_paths`: drops empty and glob entries (globs are
/// handled by [`compile_exclude_globs`]) and ensures a trailing `/` so matching is path-segment
/// aware. This makes `Content/Dev` and `Content/Dev/` behave identically and prevents a
/// sibling-prefix match (`Content/Dev` must not exclude `Content/Development`).
fn normalize_exclude_paths(patterns: &[String]) -> Vec<String> {
    patterns
        .iter()
        .filter(|p| !p.is_empty() && !is_glob_pattern(p))
        .map(|p| {
            if p.ends_with('/') {
                p.clone()
            } else {
                format!("{p}/")
            }
        })
        .collect()
}

/// Compiles the glob subset of `scan.exclude_paths` into matchers. `*`/`?` do not cross `/`;
/// `**` does. Matching is case-insensitive on Windows, mirroring `.uasset-lens-ignore`.
fn compile_exclude_globs(patterns: &[String]) -> anyhow::Result<Vec<globset::GlobMatcher>> {
    patterns
        .iter()
        .filter(|p| is_glob_pattern(p))
        .map(|p| {
            globset::GlobBuilder::new(p)
                .literal_separator(true)
                .case_insensitive(cfg!(windows))
                .build()
                .with_context(|| format!("invalid glob in scan.exclude_paths: '{p}'"))
                .map(|g| g.compile_matcher())
        })
        .collect()
}

/// Returns whether the directory at `rel` (project-relative, forward-slash normalized) is
/// excluded. `normalized` must come from [`normalize_exclude_paths`]. A trailing slash is
/// appended to `rel` so a directory matches its own `Dir/` pattern, not only nested subdirs.
fn dir_excluded(rel: &str, normalized: &[String]) -> bool {
    if rel.is_empty() {
        return false;
    }
    let rel_dir = format!("{rel}/");
    normalized.iter().any(|p| rel_dir.starts_with(p.as_str()))
}

pub fn handle_scan(
    project_dir: &Path,
    db_path: &Path,
    format: &FormatKind,
    cfg: &crate::config::ConfigFile,
    opts: &ScanOptions<'_>,
) -> anyhow::Result<i32> {
    let use_color = crate::use_color(format, opts.no_color);

    if let Some(parent) = db_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }

    let mut db = uasset_lens_asset_db::AssetDb::open(db_path).context("Failed to open database")?;

    // Fail fast before expensive walkdir if the requested baseline doesn't exist.
    let loaded_baseline = if let Some(name) = opts.diff_from {
        Some(db.load_baseline(name).map_err(|e| match e {
            uasset_lens_asset_db::DbError::BaselineNotFound(_) => anyhow::anyhow!(
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

    let excluded = normalize_exclude_paths(&cfg.scan.exclude_paths);
    // Glob exclude patterns are matched per-file; they cannot safely prune directories. Compiled
    // up front so a malformed glob fails before the walk.
    let exclude_globs = compile_exclude_globs(&cfg.scan.exclude_paths)?;
    // Load `.uasset-lens-ignore` up front so a malformed pattern fails before the walk.
    let ignore_rules = crate::ignore::IgnoreRules::load(project_dir)?;

    let effective_diff = opts.diff || opts.diff_from.is_some();

    // Capture per-asset metrics before upsert so we can compute the diff afterwards.
    let (old_bp, old_sizes) = if effective_diff {
        let bp: HashMap<uasset_lens_shared::AssetPath, (u32, u32)> = db
            .all_blueprint_metrics()
            .context("Failed to read blueprint metrics from database")?
            .into_iter()
            .map(|r| (r.asset_path, (r.node_count, r.event_tick_count)))
            .collect();
        let sizes: HashMap<uasset_lens_shared::AssetPath, u64> = db
            .all_assets()
            .context("Failed to read assets from database")?
            .into_iter()
            .map(|r| (r.asset_path, r.file_size))
            .collect();
        (bp, sizes)
    } else {
        (HashMap::new(), HashMap::new())
    };

    let mut all_files: Vec<(PathBuf, u64)> = WalkDir::new(project_dir)
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir()
                && !excluded.is_empty()
                && let Ok(rel) = e.path().strip_prefix(project_dir)
            {
                // normalize to forward slashes for cross-platform comparison
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                if dir_excluded(&rel_str, &excluded) {
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

    // Drop files excluded by glob `scan.exclude_paths` patterns or by `.uasset-lens-ignore`
    // (prefix exclude_paths are already pruned during the walk). Excluded files are treated as
    // absent: never indexed, and any previously-indexed path becomes stale on rescan.
    if !exclude_globs.is_empty() || !ignore_rules.is_empty() {
        all_files.retain(|(p, _)| {
            let rel = p
                .strip_prefix(project_dir)
                .map(|r| r.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            !exclude_globs.iter().any(|g| g.is_match(&rel)) && !ignore_rules.is_ignored(&rel)
        });
    }

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

    if !opts.quiet {
        eprintln!(
            "{}",
            scan_header(
                &content_root,
                opts.full_scan,
                total_file_count,
                paths_to_scan.len()
            )
        );
    }

    let progress = if should_show_progress(
        format,
        opts.quiet,
        opts.no_progress,
        std::io::stderr().is_terminal(),
    ) {
        let pb = indicatif::ProgressBar::new(paths_to_scan.len() as u64);
        pb.set_style(
            indicatif::ProgressStyle::with_template(
                "  {spinner} [{elapsed_precise}] [{bar:30}] {pos}/{len} files (ETA {eta})",
            )
            .unwrap_or_else(|_| indicatif::ProgressStyle::default_bar())
            .progress_chars("=>-"),
        );
        // Redraw on a timer so the spinner and elapsed time animate even while a single large
        // file is being parsed (between per-file increments).
        pb.enable_steady_tick(std::time::Duration::from_millis(100));
        pb
    } else {
        indicatif::ProgressBar::hidden()
    };

    let result =
        uasset_lens_scanner::scan_files_with_progress(&paths_to_scan, &content_root, || {
            progress.inc(1);
        });
    progress.finish_and_clear();

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

    // Print per-category scan results before prompting for stale removal. Text/GitHub-Actions
    // only: in JSON mode these human-readable lines would corrupt the single-value stdout (the
    // JSON object already carries new/updated/removed counts).
    if !opts.quiet
        && !opts.suppress_report
        && matches!(format, FormatKind::Text | FormatKind::GithubActions)
    {
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
                match uasset_lens_shared::AssetPath::from_fs_path(&content_root, path) {
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

    // When the stdout report is suppressed (e.g. `check`'s auto-scan), stale removals still
    // mutate the DB. Surface them on stderr so an unexpected mass deletion is not silent.
    if opts.suppress_report && removed_count > 0 {
        eprintln!("  {removed_count} stale record(s) removed during scan");
    }

    let snapshot_id = db
        .record_scan_snapshot()
        .context("Failed to record scan snapshot")?;
    // Stamp the scanner (binary) version into the DB for `doctor`'s compat check.
    db.set_scanner_version(env!("CARGO_PKG_VERSION"))
        .context("Failed to record scanner version")?;
    if let Some(name) = opts.save_baseline {
        db.save_baseline(name, snapshot_id)
            .context("Failed to save baseline")?;
        if !opts.quiet {
            eprintln!("Baseline '{}' saved.", name);
        }
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
            opts.quiet,
        );
    }

    match format {
        FormatKind::Sarif => return Err(crate::sarif_not_supported()),
        FormatKind::Json => {
            if !opts.suppress_report {
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
                crate::emit_json(&output, "Failed to serialize scan output to JSON")?;
            }
        }
        FormatKind::Text | FormatKind::GithubActions => {
            if !opts.suppress_report {
                println!();
                println!(
                    "  {} {} assets total, {} record(s) cleaned, {} skipped (parse error)",
                    sym("✓", "\x1b[32m✓\x1b[0m", use_color),
                    assets_total,
                    removed_count,
                    result.skipped.len()
                );
            }

            if !opts.quiet && !result.skipped.is_empty() {
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

/// An asset excluded by a `--dry-run` preview, with the rule source that excluded it.
struct IgnoredEntry {
    path: String,
    /// `exclude_paths` (TOML prefix or glob) or `.uasset-lens-ignore`.
    source: &'static str,
}

/// A `--dry-run` classification: assets that would be scanned, and those excluded.
struct DryRunPlan {
    scanned: Vec<String>,
    ignored: Vec<IgnoredEntry>,
}

/// Walks the project and partitions assets into scanned vs. excluded, without touching the
/// database. `exclude_paths` covers both prefix and glob patterns; `.uasset-lens-ignore`
/// covers ignore-file rules.
fn classify_for_dry_run(
    project_dir: &Path,
    cfg: &crate::config::ConfigFile,
) -> anyhow::Result<DryRunPlan> {
    let prefixes = normalize_exclude_paths(&cfg.scan.exclude_paths);
    let exclude_globs = compile_exclude_globs(&cfg.scan.exclude_paths)?;
    let ignore_rules = crate::ignore::IgnoreRules::load(project_dir)?;

    let mut scanned: Vec<String> = Vec::new();
    let mut ignored: Vec<IgnoredEntry> = Vec::new();

    for entry in WalkDir::new(project_dir).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let is_asset = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("uasset") || ext.eq_ignore_ascii_case("umap"))
            .unwrap_or(false);
        if !is_asset {
            continue;
        }
        let rel = path
            .strip_prefix(project_dir)
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();

        if prefixes.iter().any(|p| rel.starts_with(p.as_str()))
            || exclude_globs.iter().any(|g| g.is_match(&rel))
        {
            ignored.push(IgnoredEntry {
                path: rel,
                source: "exclude_paths",
            });
        } else if ignore_rules.is_ignored(&rel) {
            ignored.push(IgnoredEntry {
                path: rel,
                source: ".uasset-lens-ignore",
            });
        } else {
            scanned.push(rel);
        }
    }

    scanned.sort();
    ignored.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(DryRunPlan { scanned, ignored })
}

/// Previews which assets would be scanned vs. excluded without writing to the database.
pub fn handle_scan_dry_run(
    project_dir: &Path,
    cfg: &crate::config::ConfigFile,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    let plan = classify_for_dry_run(project_dir, cfg)?;

    match format {
        FormatKind::Sarif => return Err(crate::sarif_not_supported()),
        FormatKind::Json => {
            let out = serde_json::json!({
                "scanned": plan.scanned,
                "ignored": plan.ignored.iter().map(|e| &e.path).collect::<Vec<_>>(),
            });
            crate::emit_json(&out, "Failed to serialize dry-run output to JSON")?;
        }
        FormatKind::GithubActions | FormatKind::Text => {
            for path in &plan.scanned {
                println!("  {path}");
            }
            for entry in &plan.ignored {
                println!("  {}  [ignored: {}]", entry.path, entry.source);
            }
            println!();
            println!(
                "  Dry run: {} to scan, {} ignored (0 written)",
                plan.scanned.len(),
                plan.ignored.len()
            );
        }
    }
    Ok(0)
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
    use uasset_lens_shared::{AssetPath, AssetType};

    #[test]
    fn normalize_exclude_paths_should_add_trailing_slash() {
        let out =
            normalize_exclude_paths(&["Content/Dev".to_string(), "Content/Test/".to_string()]);
        assert_eq!(out, vec!["Content/Dev/", "Content/Test/"]);
    }

    #[test]
    fn normalize_exclude_paths_should_drop_empty() {
        let out = normalize_exclude_paths(&["".to_string(), "Content/Dev/".to_string()]);
        assert_eq!(out, vec!["Content/Dev/"]);
    }

    #[test]
    fn dir_excluded_should_match_trailing_slash_pattern() {
        // The bug: a trailing-slash pattern must exclude the directory itself, not only subdirs.
        let pats = normalize_exclude_paths(&["Content/Dev/".to_string()]);
        assert!(dir_excluded("Content/Dev", &pats));
    }

    #[test]
    fn dir_excluded_should_match_no_slash_pattern_consistently() {
        let pats = normalize_exclude_paths(&["Content/Dev".to_string()]);
        assert!(dir_excluded("Content/Dev", &pats));
    }

    #[test]
    fn dir_excluded_should_not_match_sibling_prefix() {
        let pats = normalize_exclude_paths(&["Content/Dev".to_string()]);
        assert!(
            !dir_excluded("Content/Development", &pats),
            "a segment-aware pattern must not match a sibling sharing the prefix"
        );
    }

    #[test]
    fn dir_excluded_should_match_nested_subdir() {
        let pats = normalize_exclude_paths(&["Content/Dev/".to_string()]);
        assert!(dir_excluded("Content/Dev/Sub", &pats));
    }

    #[test]
    fn dir_excluded_should_return_false_for_root_and_empty_patterns() {
        let pats = normalize_exclude_paths(&["Content/Dev/".to_string()]);
        assert!(!dir_excluded("", &pats));
        assert!(!dir_excluded("Content/Maps", &[]));
    }

    #[test]
    fn is_glob_pattern_should_detect_metacharacters() {
        assert!(is_glob_pattern("Content/**/Test*.uasset"));
        assert!(is_glob_pattern("Content/T?.uasset"));
        assert!(!is_glob_pattern("Content/Dev/"));
    }

    #[test]
    fn should_show_progress_should_be_true_for_text_format_on_a_tty() {
        assert!(should_show_progress(&FormatKind::Text, false, false, true));
    }

    #[test]
    fn should_show_progress_should_be_false_when_suppressed() {
        assert!(
            !should_show_progress(&FormatKind::Text, false, false, false),
            "non-TTY stderr must suppress the bar"
        );
        assert!(
            !should_show_progress(&FormatKind::Text, true, false, true),
            "--quiet must suppress the bar"
        );
        assert!(
            !should_show_progress(&FormatKind::Text, false, true, true),
            "--no-progress must suppress the bar"
        );
        assert!(
            !should_show_progress(&FormatKind::Json, false, false, true),
            "JSON output must suppress the bar"
        );
    }

    #[test]
    fn normalize_exclude_paths_should_skip_glob_patterns() {
        let out = normalize_exclude_paths(&[
            "Content/Dev/".to_string(),
            "Content/**/Test*.uasset".to_string(),
        ]);
        assert_eq!(out, vec!["Content/Dev/"]);
    }

    #[test]
    fn compile_exclude_globs_should_compile_only_globs() {
        let globs = compile_exclude_globs(&[
            "Content/Dev/".to_string(),
            "Content/**/Test*.uasset".to_string(),
        ])
        .unwrap();
        assert_eq!(
            globs.len(),
            1,
            "only the glob entry should compile to a matcher"
        );
        assert!(globs[0].is_match("Content/A/TestRig.uasset"));
        assert!(!globs[0].is_match("Content/A/BP_Real.uasset"));
    }

    #[test]
    fn compile_exclude_globs_should_error_on_invalid_glob() {
        // Contains `*` (routes to the glob branch) with an unterminated character class.
        let result = compile_exclude_globs(&["Content/*[bad".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn classify_for_dry_run_should_separate_scanned_and_ignored() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_dryrun_{}", std::process::id()));
        let content = dir.join("Content");
        for sub in ["Maps", "Dev", "QA", "Junk"] {
            std::fs::create_dir_all(content.join(sub)).unwrap();
        }
        // dry-run does not parse, so empty files suffice.
        std::fs::write(content.join("Maps/BP_Keep.uasset"), b"").unwrap();
        std::fs::write(content.join("Dev/BP_Tool.uasset"), b"").unwrap();
        std::fs::write(content.join("QA/TestRig.uasset"), b"").unwrap();
        std::fs::write(content.join("Junk/X.uasset"), b"").unwrap();
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[scan]\nexclude_paths = [\"Content/Dev/\", \"Content/**/Test*.uasset\"]\n",
        )
        .unwrap();
        std::fs::write(dir.join(".uasset-lens-ignore"), "Content/Junk/\n").unwrap();
        let cfg = crate::config::load_config(&dir);

        let plan = classify_for_dry_run(&dir, &cfg).unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(plan.scanned, vec!["Content/Maps/BP_Keep.uasset"]);
        let by_path: std::collections::HashMap<&str, &str> = plan
            .ignored
            .iter()
            .map(|e| (e.path.as_str(), e.source))
            .collect();
        assert_eq!(plan.ignored.len(), 3);
        assert_eq!(
            by_path.get("Content/Dev/BP_Tool.uasset"),
            Some(&"exclude_paths")
        );
        assert_eq!(
            by_path.get("Content/QA/TestRig.uasset"),
            Some(&"exclude_paths")
        );
        assert_eq!(
            by_path.get("Content/Junk/X.uasset"),
            Some(&".uasset-lens-ignore")
        );
    }

    #[test]
    fn classify_for_dry_run_should_error_on_invalid_glob() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_dryrun_bad_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[scan]\nexclude_paths = [\"Content/*[bad\"]\n",
        )
        .unwrap();
        let cfg = crate::config::load_config(&dir);
        let result = classify_for_dry_run(&dir, &cfg);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_err());
    }

    #[test]
    fn handle_scan_dry_run_should_return_0_and_not_create_db() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_dryrun_nodb_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("Content")).unwrap();
        std::fs::write(dir.join("Content/BP_A.uasset"), b"").unwrap();

        let result = handle_scan_dry_run(&dir, &Default::default(), &FormatKind::Text).unwrap();
        let db_created = dir.join(".uasset-lens").exists();
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(result, 0);
        assert!(
            !db_created,
            "dry-run must not create the .uasset-lens database directory"
        );
    }

    #[test]
    fn handle_scan_should_exclude_files_matching_glob_pattern() {
        let (dir, db_path) = test_db_in_tempdir("scan_glob_exclude");
        let fixture = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/valid/BP_Simple.uasset"
        ));
        let qa = dir.join("Content").join("QA");
        let maps = dir.join("Content").join("Maps");
        std::fs::create_dir_all(&qa).unwrap();
        std::fs::create_dir_all(&maps).unwrap();
        std::fs::copy(&fixture, qa.join("TestRig.uasset")).unwrap();
        std::fs::copy(&fixture, maps.join("BP_Level.uasset")).unwrap();
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[scan]\nexclude_paths = [\"Content/**/Test*.uasset\"]\n",
        )
        .unwrap();
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
            },
        )
        .unwrap();

        let db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
        let paths: Vec<String> = db
            .all_assets()
            .unwrap()
            .iter()
            .map(|a| a.asset_path.as_str().to_owned())
            .collect();
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(
            paths.len(),
            1,
            "the glob-matched file must be excluded; got {paths:?}"
        );
        assert!(
            paths.iter().any(|p| p.contains("BP_Level")),
            "non-matching asset should be indexed: {paths:?}"
        );
        assert!(
            !paths.iter().any(|p| p.contains("TestRig")),
            "glob-matched asset must not be indexed: {paths:?}"
        );
    }

    #[test]
    fn handle_scan_should_exclude_direct_file_under_trailing_slash_dir() {
        let (dir, db_path) = test_db_in_tempdir("scan_exclude_direct");
        let fixture = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/valid/BP_Simple.uasset"
        ));
        let dev = dir.join("Content").join("Dev");
        let maps = dir.join("Content").join("Maps");
        std::fs::create_dir_all(&dev).unwrap();
        std::fs::create_dir_all(&maps).unwrap();
        std::fs::copy(&fixture, dev.join("BP_Direct.uasset")).unwrap();
        std::fs::copy(&fixture, maps.join("BP_Keep.uasset")).unwrap();
        // Trailing-slash directory pattern, as documented in config.md.
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[scan]\nexclude_paths = [\"Content/Dev/\"]\n",
        )
        .unwrap();
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
            },
        )
        .unwrap();

        let db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
        let paths: Vec<String> = db
            .all_assets()
            .unwrap()
            .iter()
            .map(|a| a.asset_path.as_str().to_owned())
            .collect();
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(
            paths.len(),
            1,
            "the direct file under Content/Dev/ must be excluded; got {paths:?}"
        );
        assert!(
            paths.iter().any(|p| p.contains("BP_Keep")),
            "non-excluded asset should be indexed: {paths:?}"
        );
        assert!(
            !paths.iter().any(|p| p.contains("BP_Direct")),
            "direct file under excluded dir must not be indexed: {paths:?}"
        );
    }

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

    fn make_meta(
        asset_path: &str,
        file_path: PathBuf,
        mtime: u64,
    ) -> uasset_lens_scanner::AssetMetadata {
        uasset_lens_scanner::AssetMetadata {
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
            },
        )
        .unwrap();
        assert_eq!(result, 0);

        let db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
            },
        )
        .unwrap();

        let db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
        assert!(
            db.all_assets().unwrap().is_empty(),
            "excluded path files should not be indexed in the database"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_scan_should_not_index_files_matched_by_ignore_file() {
        let (dir, db_path) = test_db_in_tempdir("scan_ignore");
        let fixture = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/valid/BP_Simple.uasset"
        ));
        let dev = dir.join("Content").join("Dev");
        let maps = dir.join("Content").join("Maps");
        std::fs::create_dir_all(&dev).unwrap();
        std::fs::create_dir_all(&maps).unwrap();
        std::fs::copy(&fixture, dev.join("BP_Ignored.uasset")).unwrap();
        std::fs::copy(&fixture, maps.join("BP_Kept.uasset")).unwrap();
        std::fs::write(dir.join(".uasset-lens-ignore"), "Content/Dev/\n").unwrap();

        let _ = handle_scan(
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
            },
        )
        .unwrap();

        let db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
        let paths: Vec<String> = db
            .all_assets()
            .unwrap()
            .iter()
            .map(|a| a.asset_path.as_str().to_owned())
            .collect();
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(
            paths.len(),
            1,
            "only the non-ignored asset should be indexed; got {paths:?}"
        );
        assert!(
            paths.iter().any(|p| p.contains("BP_Kept")),
            "kept asset should be indexed: {paths:?}"
        );
        assert!(
            !paths.iter().any(|p| p.contains("BP_Ignored")),
            "ignored asset must not be indexed: {paths:?}"
        );
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
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
            let db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
            },
        )
        .unwrap();

        assert_eq!(
            result, 1,
            "exit code should be 1 when stale assets are removed"
        );

        let db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
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
            let db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
            },
        )
        .unwrap();

        assert_eq!(
            result, 0,
            "exit code should be 0 when removal is not confirmed"
        );

        let db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
            },
        )
        .unwrap();
        assert_eq!(result, 0);

        let db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
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
                quiet: false,
                suppress_report: false,
                no_color: false,
                no_progress: false,
            },
        );
        assert!(
            result.is_err(),
            "diff-from unknown baseline should return an error"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
