use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::FormatKind;
use crate::time::format_utc;

#[derive(serde::Serialize)]
pub(super) struct BpRegressionEntry {
    pub path: String,
    pub old_node_count: u32,
    pub new_node_count: u32,
    pub old_event_tick_count: u32,
    pub new_event_tick_count: u32,
}

#[derive(serde::Serialize)]
pub(super) struct SizeIncreaseEntry {
    pub path: String,
    pub old_size: u64,
    pub new_size: u64,
    pub pct_increase: u64,
}

#[derive(serde::Serialize)]
pub(super) struct ScanDiffOutput {
    pub prev_scanned_at: Option<u64>,
    pub baseline_name: Option<String>,
    pub new_assets: Vec<String>,
    pub deleted_assets: Vec<String>,
    pub regressions: Vec<BpRegressionEntry>,
    pub size_increases: Vec<SizeIncreaseEntry>,
}

pub(super) struct DiffInput<'a> {
    pub assets: &'a [uasset_lens_scanner::AssetMetadata],
    pub old_bp: HashMap<uasset_lens_shared::AssetPath, (u32, u32)>,
    pub old_sizes: HashMap<uasset_lens_shared::AssetPath, u64>,
    pub db_files: &'a HashSet<PathBuf>,
    pub stale: &'a [PathBuf],
    pub content_root: &'a Path,
    pub project_dir: &'a Path,
    pub threshold: u64,
    pub diff_from: Option<&'a str>,
    pub prev_scanned_at: Option<u64>,
}

pub(super) fn print_diff(
    input: DiffInput<'_>,
    format: &FormatKind,
    use_color: bool,
    quiet: bool,
) -> anyhow::Result<i32> {
    crate::maybe_hint_github_actions(format, quiet);

    let regressions = compute_regressions(input.assets.iter(), &input.old_bp);
    let size_increases =
        compute_size_increases(input.assets.iter(), &input.old_sizes, input.threshold);

    let new_asset_paths: Vec<String> = input
        .assets
        .iter()
        .filter(|a| !input.db_files.contains(&a.file_path))
        .map(|a| a.asset_path.as_str().to_owned())
        .collect();

    let deleted_asset_paths: Vec<String> = input
        .stale
        .iter()
        .filter_map(|p| uasset_lens_shared::AssetPath::from_fs_path(input.content_root, p).ok())
        .map(|ap| ap.as_str().to_owned())
        .collect();

    let has_regressions = !regressions.is_empty();
    let has_size_increases = !size_increases.is_empty();

    match format {
        FormatKind::Sarif => return Err(crate::sarif_not_supported()),
        FormatKind::GithubActions => {
            let path_lookup: HashMap<&str, &std::path::Path> = input
                .assets
                .iter()
                .map(|m| (m.asset_path.as_str(), m.file_path.as_path()))
                .collect();
            for p in &new_asset_paths {
                println!("::notice title=NewAsset::{p}");
            }
            for p in &deleted_asset_paths {
                println!("::warning title=DeletedAsset::{p}");
            }
            for r in &regressions {
                let rel = path_lookup
                    .get(r.path.as_str())
                    .map(|&fp| crate::rel_path_for_annotation(fp, input.project_dir))
                    .unwrap_or_default();
                let node_delta = r.new_node_count - r.old_node_count;
                let msg = format!(
                    "node_count {} \u{2192} {} (+{node_delta})",
                    r.old_node_count, r.new_node_count
                );
                println!(
                    "{}",
                    crate::format_gh_annotation("error", &rel, "BlueprintRegression", &msg)
                );
            }
            for s in &size_increases {
                let rel = path_lookup
                    .get(s.path.as_str())
                    .map(|&fp| crate::rel_path_for_annotation(fp, input.project_dir))
                    .unwrap_or_default();
                let msg = format!(
                    "{} \u{2192} {} (+{}%)",
                    crate::format_size(s.old_size),
                    crate::format_size(s.new_size),
                    s.pct_increase
                );
                println!(
                    "{}",
                    crate::format_gh_annotation("error", &rel, "AssetSizeIncrease", &msg)
                );
            }
        }
        FormatKind::Json => {
            let out = ScanDiffOutput {
                prev_scanned_at: input.prev_scanned_at,
                baseline_name: input.diff_from.map(|s| s.to_owned()), // clone required: ScanDiffOutput owns the name
                new_assets: new_asset_paths,
                deleted_assets: deleted_asset_paths,
                regressions,
                size_increases,
            };
            crate::emit_json(&out, "Failed to serialize diff output to JSON")?;
        }
        FormatKind::Text => {
            println!();
            match (input.prev_scanned_at, input.diff_from) {
                (Some(ts), Some(name)) => {
                    println!("Diff vs baseline \"{}\" ({}):", name, format_utc(ts))
                }
                (Some(ts), None) => println!("Diff vs previous scan ({}):", format_utc(ts)),
                (None, _) => println!("Diff: (no previous scan to compare against)"),
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
                    super::sym("!", "\x1b[31m!\x1b[0m", use_color),
                    regressions.len()
                );
                for r in &regressions {
                    let node_delta = r.new_node_count - r.old_node_count;
                    let tick_delta = r.new_event_tick_count as i64 - r.old_event_tick_count as i64;
                    let tick_suffix = if tick_delta > 0 {
                        format!("  [EventTick +{tick_delta}]")
                    } else {
                        String::new()
                    };
                    println!(
                        "      {}  {} \u{2192} {} nodes  (+{node_delta}){tick_suffix}",
                        r.path, r.old_node_count, r.new_node_count
                    );
                }
            }
            if !size_increases.is_empty() {
                println!(
                    "  ^ {} asset(s) grew by \u{2265}{}%:",
                    size_increases.len(),
                    input.threshold
                );
                for s in &size_increases {
                    println!(
                        "      {}  {} \u{2192} {} (+{}%)",
                        s.path,
                        crate::format_size(s.old_size),
                        crate::format_size(s.new_size),
                        s.pct_increase
                    );
                }
            }
        }
    }

    // In GH Actions mode size increases are annotated as ::error → exit 1.
    // In Text/JSON mode they are informational only → exit 0.
    let exit_1 =
        has_regressions || (matches!(format, FormatKind::GithubActions) && has_size_increases);
    if exit_1 { Ok(1) } else { Ok(0) }
}

pub(super) fn compute_regressions<'a>(
    assets: impl Iterator<Item = &'a uasset_lens_scanner::AssetMetadata>,
    old_bp: &HashMap<uasset_lens_shared::AssetPath, (u32, u32)>,
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

pub(super) fn compute_size_increases<'a>(
    assets: impl Iterator<Item = &'a uasset_lens_scanner::AssetMetadata>,
    old_sizes: &HashMap<uasset_lens_shared::AssetPath, u64>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use uasset_lens_shared::{AssetPath, AssetType};

    fn make_bp_meta(
        asset_path: &str,
        node_count: u32,
        event_tick_count: u32,
    ) -> uasset_lens_scanner::AssetMetadata {
        let bm = uasset_lens_scanner::BlueprintMetrics {
            node_count,
            event_tick_count,
            cast_count: 0,
            dependency_depth: 0,
        };
        uasset_lens_scanner::AssetMetadata {
            file_path: PathBuf::from(format!("/proj/Content/{}.uasset", asset_path)),
            file_size: 1024,
            last_modified: 0,
            blueprint_metrics: Some(bm),
            ..uasset_lens_scanner::make_meta(asset_path, AssetType::Blueprint)
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
        let meta = uasset_lens_scanner::AssetMetadata {
            file_path: PathBuf::from("/proj/Content/T_Rock.uasset"),
            file_size: 1200,
            last_modified: 0,
            ..uasset_lens_scanner::make_meta("/Game/T_Rock", AssetType::Texture2D)
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
        let meta = uasset_lens_scanner::AssetMetadata {
            file_path: PathBuf::from("/proj/Content/T_Rock.uasset"),
            file_size: 1050,
            last_modified: 0,
            ..uasset_lens_scanner::make_meta("/Game/T_Rock", AssetType::Texture2D)
        };

        let result = compute_size_increases(std::iter::once(&meta), &old_sizes, 10);

        assert!(result.is_empty());
    }

    #[test]
    fn compute_size_increases_should_ignore_asset_with_no_previous_size() {
        let old_sizes: HashMap<AssetPath, u64> = HashMap::new();
        let meta = uasset_lens_scanner::AssetMetadata {
            file_path: PathBuf::from("/proj/Content/T_New.uasset"),
            file_size: 5000,
            last_modified: 0,
            ..uasset_lens_scanner::make_meta("/Game/T_New", AssetType::Texture2D)
        };

        let result = compute_size_increases(std::iter::once(&meta), &old_sizes, 10);

        assert!(result.is_empty());
    }
}
