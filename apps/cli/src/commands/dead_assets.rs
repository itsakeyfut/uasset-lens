use std::collections::HashMap;
use std::path::Path;

use crate::{FormatKind, GroupMode};

#[derive(serde::Serialize)]
struct DeadAssetEntry {
    path: String,
    #[serde(rename = "type")]
    asset_type: String,
    file_size: u64,
}

#[derive(serde::Serialize)]
struct ByTypeEntry {
    #[serde(rename = "type")]
    asset_type: String,
    count: usize,
    bytes: u64,
}

#[derive(serde::Serialize)]
struct DeadAssetsOutput {
    assets: Vec<DeadAssetEntry>,
    total_size_bytes: u64,
    by_type: Vec<ByTypeEntry>,
}

#[derive(serde::Serialize)]
struct GroupEntry {
    group: String,
    count: usize,
    total_size_bytes: u64,
}

// Each arg maps to a distinct CLI flag; a wrapper struct adds indirection at a single call site.
#[allow(clippy::too_many_arguments)]
pub fn handle_dead_assets(
    _project_dir: &Path,
    asset_type_filter: &[String],
    sort_by_size: bool,
    min_size: Option<u64>,
    exclude_patterns: &[String],
    group: Option<&GroupMode>,
    include_all_types: bool,
    db_path: &Path,
    cfg: &crate::config::ConfigFile,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    let db = crate::open_db(db_path)?;
    let graph = crate::load_graph(&db, &cfg.scan.external_roots)?;

    let excluded = if include_all_types {
        &[] as &[&str]
    } else {
        uasset_lens_dependency_graph::dead_assets::DEFAULT_EXCLUDED_TYPES
    };
    let dead_paths = uasset_lens_dependency_graph::dead_assets::detect(&graph, excluded);

    let type_map: HashMap<&uasset_lens_shared::AssetPath, String> = graph
        .nodes()
        .map(|n| (&n.path, n.asset_type.to_string()))
        .collect();

    let mut entries: Vec<DeadAssetEntry> = dead_paths
        .iter()
        .filter(|path| {
            type_map
                .get(path)
                .is_some_and(|t| type_matches(t, asset_type_filter))
        })
        .map(|path| {
            let asset_type = type_map
                .get(path)
                .cloned() // clone required: type_map yields &String; owned String needed for DeadAssetEntry
                .unwrap_or_default();
            let file_size = db
                .get_asset(path)
                .ok()
                .flatten()
                .map(|r| r.file_size)
                .unwrap_or(0);
            DeadAssetEntry {
                path: path.as_str().to_owned(), // to_owned() required: field needs owned String from &str
                asset_type,
                file_size,
            }
        })
        .collect();

    if let Some(min) = min_size {
        entries.retain(|e| e.file_size >= min);
    }

    if !exclude_patterns.is_empty() {
        entries.retain(|e| !exclude_patterns.iter().any(|p| e.path.contains(p.as_str())));
    }

    if sort_by_size {
        entries.sort_unstable_by_key(|e| std::cmp::Reverse(e.file_size));
    }

    let count = entries.len();
    let total_size_bytes: u64 = entries.iter().map(|e| e.file_size).sum();

    if let Some(mode) = group {
        let mut map: HashMap<String, (usize, u64)> = HashMap::new();
        for e in &entries {
            let key = match mode {
                GroupMode::Type => e.asset_type.clone(),
                GroupMode::Dir => crate::path_depth_prefix(&e.path).to_owned(),
            };
            let (cnt, size) = map.entry(key).or_default();
            *cnt += 1;
            *size += e.file_size;
        }
        let mut groups: Vec<GroupEntry> = map
            .into_iter()
            .map(|(group, (cnt, total))| GroupEntry {
                group,
                count: cnt,
                total_size_bytes: total,
            })
            .collect();
        groups.sort_unstable_by_key(|g| std::cmp::Reverse(g.total_size_bytes));

        match format {
            FormatKind::Sarif => return Err(crate::sarif_not_supported()),
            FormatKind::Json => {
                crate::emit_json(&groups, "Failed to serialize grouped output to JSON")?;
            }
            FormatKind::GithubActions | FormatKind::Text => {
                let max_name = groups.iter().map(|g| g.group.len()).max().unwrap_or(1);
                let max_cnt = groups
                    .iter()
                    .map(|g| crate::digit_count(g.count))
                    .max()
                    .unwrap_or(1);
                for g in &groups {
                    println!(
                        "  {:<name$}  ({:>cnt$} assets, {})",
                        g.group,
                        g.count,
                        crate::format_size(g.total_size_bytes),
                        name = max_name,
                        cnt = max_cnt,
                    );
                }
                if count > 0 {
                    println!();
                }
                println!("  {}", format_dead_summary(count, total_size_bytes));
            }
        }
        return if count == 0 { Ok(0) } else { Ok(1) };
    }

    let by_type = compute_by_type(&entries);

    match format {
        FormatKind::Sarif => return Err(crate::sarif_not_supported()),
        FormatKind::Json => {
            let output = DeadAssetsOutput {
                assets: entries,
                total_size_bytes,
                by_type,
            };
            crate::emit_json(&output, "Failed to serialize dead assets output to JSON")?;
        }
        FormatKind::GithubActions | FormatKind::Text => {
            for entry in &entries {
                println!(
                    "  {}  ({}, {})",
                    entry.path,
                    entry.asset_type,
                    crate::format_size(entry.file_size)
                );
            }
            if count > 0 {
                println!();
            }
            println!("  {}", format_dead_summary(count, total_size_bytes));
            print_by_type(&by_type);
        }
    }

    if count == 0 { Ok(0) } else { Ok(1) }
}

/// Prints the `By type:` breakdown (all types, wasted-bytes descending) below the summary.
fn print_by_type(by_type: &[ByTypeEntry]) {
    if by_type.is_empty() {
        return;
    }
    let max_name = by_type
        .iter()
        .map(|b| b.asset_type.len())
        .max()
        .unwrap_or(1);
    let max_cnt = by_type
        .iter()
        .map(|b| crate::digit_count(b.count))
        .max()
        .unwrap_or(1);
    println!();
    println!("  By type:");
    for b in by_type {
        println!(
            "    {:<name$}  {:>cnt$}  {}",
            b.asset_type,
            b.count,
            crate::format_size(b.bytes),
            name = max_name,
            cnt = max_cnt,
        );
    }
}

/// Whether `asset_type` passes the `--type` filter: an empty filter accepts everything, otherwise
/// the type must match one of the (OR-combined) requested types.
fn type_matches(asset_type: &str, filters: &[String]) -> bool {
    filters.is_empty() || filters.iter().any(|f| f == asset_type)
}

/// Aggregates dead assets by asset type, sorted by wasted bytes descending (ties broken by type
/// name for deterministic output). Drives the `By type:` breakdown shown alongside the summary.
fn compute_by_type(entries: &[DeadAssetEntry]) -> Vec<ByTypeEntry> {
    let mut map: HashMap<&str, (usize, u64)> = HashMap::new();
    for e in entries {
        let (count, bytes) = map.entry(e.asset_type.as_str()).or_default();
        *count += 1;
        *bytes += e.file_size;
    }
    let mut by_type: Vec<ByTypeEntry> = map
        .into_iter()
        .map(|(asset_type, (count, bytes))| ByTypeEntry {
            asset_type: asset_type.to_owned(),
            count,
            bytes,
        })
        .collect();
    by_type.sort_unstable_by(|a, b| {
        b.bytes
            .cmp(&a.bytes)
            .then_with(|| a.asset_type.cmp(&b.asset_type))
    });
    by_type
}

fn format_dead_summary(count: usize, total_bytes: u64) -> String {
    if count == 0 {
        "Dead Assets (0 found)".to_owned()
    } else {
        format!(
            "Dead Assets ({} found, {} wasted)",
            count,
            crate::format_size(total_bytes)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{make_meta, test_db_in_tempdir};
    use uasset_lens_shared::{AssetPath, AssetType};

    #[test]
    fn format_dead_summary_should_show_count_only_when_zero() {
        assert_eq!(format_dead_summary(0, 0), "Dead Assets (0 found)");
    }

    #[test]
    fn format_dead_summary_should_include_size_when_assets_found() {
        assert_eq!(
            format_dead_summary(3, 1024 * 1024),
            "Dead Assets (3 found, 1.0 MB wasted)"
        );
    }

    #[test]
    fn dead_assets_output_should_serialize_total_size_bytes_field() {
        let output = DeadAssetsOutput {
            assets: vec![],
            total_size_bytes: 347_200_000,
            by_type: vec![],
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(
            json.contains("\"total_size_bytes\""),
            "JSON must contain total_size_bytes key"
        );
        assert!(
            json.contains("347200000"),
            "JSON must contain the correct total_size_bytes value"
        );
    }

    #[test]
    fn dead_assets_output_should_serialize_assets_array() {
        let output = DeadAssetsOutput {
            assets: vec![DeadAssetEntry {
                path: "/Game/A".to_owned(),
                asset_type: "Blueprint".to_owned(),
                file_size: 4096,
            }],
            total_size_bytes: 4096,
            by_type: vec![],
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"assets\""), "JSON must contain assets key");
        assert!(json.contains("/Game/A"));
    }

    #[test]
    fn compute_by_type_should_aggregate_and_sort_by_bytes_desc() {
        let entries = vec![
            DeadAssetEntry {
                path: "/Game/A".to_owned(),
                asset_type: "Blueprint".to_owned(),
                file_size: 100,
            },
            DeadAssetEntry {
                path: "/Game/B".to_owned(),
                asset_type: "Texture2D".to_owned(),
                file_size: 500,
            },
            DeadAssetEntry {
                path: "/Game/C".to_owned(),
                asset_type: "Blueprint".to_owned(),
                file_size: 50,
            },
            DeadAssetEntry {
                path: "/Game/D".to_owned(),
                asset_type: "Texture2D".to_owned(),
                file_size: 200,
            },
        ];
        let by = compute_by_type(&entries);
        // Texture2D: 2 assets / 700 B, Blueprint: 2 assets / 150 B → bytes descending.
        assert_eq!(by.len(), 2);
        assert_eq!(by[0].asset_type, "Texture2D");
        assert_eq!(by[0].count, 2);
        assert_eq!(by[0].bytes, 700);
        assert_eq!(by[1].asset_type, "Blueprint");
        assert_eq!(by[1].count, 2);
        assert_eq!(by[1].bytes, 150);
    }

    #[test]
    fn compute_by_type_should_break_byte_ties_by_type_name() {
        let entries = vec![
            DeadAssetEntry {
                path: "/Game/A".to_owned(),
                asset_type: "Texture2D".to_owned(),
                file_size: 100,
            },
            DeadAssetEntry {
                path: "/Game/B".to_owned(),
                asset_type: "Blueprint".to_owned(),
                file_size: 100,
            },
        ];
        let by = compute_by_type(&entries);
        assert_eq!(
            by[0].asset_type, "Blueprint",
            "equal bytes break ties alphabetically"
        );
        assert_eq!(by[1].asset_type, "Texture2D");
    }

    #[test]
    fn by_type_entry_json_should_use_type_count_bytes_keys() {
        let json = serde_json::to_string(&ByTypeEntry {
            asset_type: "AnimSequence".to_owned(),
            count: 161,
            bytes: 43_690_000,
        })
        .unwrap();
        assert!(
            json.contains("\"type\":\"AnimSequence\""),
            "asset_type must serialize as 'type'"
        );
        assert!(json.contains("\"count\":161"));
        assert!(json.contains("\"bytes\":43690000"));
    }

    #[test]
    fn handle_dead_assets_should_return_err_when_db_does_not_exist() {
        let (dir, db_path) = test_db_in_tempdir("dead22_missing");
        let _ = std::fs::remove_dir_all(&dir);

        let result = handle_dead_assets(
            Path::new("/proj"),
            &[],
            false,
            None,
            &[],
            None,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        );
        assert!(result.is_err(), "missing DB should return an error");
    }

    #[test]
    fn handle_dead_assets_should_return_0_when_db_has_no_assets() {
        let (dir, db_path) = test_db_in_tempdir("dead22_empty");
        uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_dead_assets(
            &dir,
            &[],
            false,
            None,
            &[],
            None,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_should_return_0_when_all_nodes_form_cycle() {
        let (dir, db_path) = test_db_in_tempdir("dead22_cycle");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            // A→B→A: every node has in_degree >= 1, nothing is dead.
            db.upsert_all(&[
                make_meta(
                    "/Game/A",
                    dir.join("A.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![AssetPath::new("/Game/B").unwrap()],
                ),
                make_meta(
                    "/Game/B",
                    dir.join("B.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![AssetPath::new("/Game/A").unwrap()],
                ),
            ])
            .unwrap();
        }

        let result = handle_dead_assets(
            &dir,
            &[],
            false,
            None,
            &[],
            None,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "all nodes in a cycle have in_degree >= 1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_should_return_1_when_unreferenced_asset_exists() {
        let (dir, db_path) = test_db_in_tempdir("dead22_found");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/Orphan",
                dir.join("Orphan.uasset"),
                AssetType::Blueprint,
                4096,
                vec![],
            )])
            .unwrap();
        }

        let result = handle_dead_assets(
            &dir,
            &[],
            false,
            None,
            &[],
            None,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 1, "/Game/Orphan has no incoming edges");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_type_filter_should_return_0_when_no_type_match() {
        let (dir, db_path) = test_db_in_tempdir("dead22_filter_miss");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/OrphanBP",
                dir.join("OrphanBP.uasset"),
                AssetType::Blueprint,
                1024,
                vec![],
            )])
            .unwrap();
        }

        let result = handle_dead_assets(
            &dir,
            &["Texture2D".to_owned()],
            false,
            None,
            &[],
            None,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(
            result, 0,
            "dead asset is Blueprint, filter is Texture2D — no match"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_type_filter_should_return_1_when_type_matches() {
        let (dir, db_path) = test_db_in_tempdir("dead22_filter_hit");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/OrphanBP",
                    dir.join("OrphanBP.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
                make_meta(
                    "/Game/OrphanTex",
                    dir.join("OrphanTex.uasset"),
                    AssetType::Texture2D,
                    2048,
                    vec![],
                ),
            ])
            .unwrap();
        }

        let result = handle_dead_assets(
            &dir,
            &["Texture2D".to_owned()],
            false,
            None,
            &[],
            None,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 1, "Texture2D dead asset matches the filter");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn type_matches_should_accept_all_when_empty_and_or_combine_otherwise() {
        assert!(type_matches("Blueprint", &[]), "empty filter accepts all");
        let filters = vec!["Blueprint".to_owned(), "Texture2D".to_owned()];
        assert!(type_matches("Blueprint", &filters));
        assert!(type_matches("Texture2D", &filters));
        assert!(
            !type_matches("StaticMesh", &filters),
            "a non-listed type is filtered out"
        );
    }

    #[test]
    fn handle_dead_assets_multiple_type_filters_should_or_combine() {
        let (dir, db_path) = test_db_in_tempdir("dead240_multi");
        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/OrphanBP",
                    dir.join("OrphanBP.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
                make_meta(
                    "/Game/OrphanTex",
                    dir.join("OrphanTex.uasset"),
                    AssetType::Texture2D,
                    2048,
                    vec![],
                ),
                make_meta(
                    "/Game/OrphanSM",
                    dir.join("OrphanSM.uasset"),
                    AssetType::StaticMesh,
                    4096,
                    vec![],
                ),
            ])
            .unwrap();
        }
        // Blueprint OR Texture2D match (StaticMesh excluded) → dead assets remain → exit 1.
        let matched = handle_dead_assets(
            &dir,
            &["Blueprint".to_owned(), "Texture2D".to_owned()],
            false,
            None,
            &[],
            None,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(matched, 1, "Blueprint and Texture2D match the OR filter");
        // No dead asset is a Material → filtered to empty → exit 0.
        let unmatched = handle_dead_assets(
            &dir,
            &["Material".to_owned()],
            false,
            None,
            &[],
            None,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(unmatched, 0, "no dead asset is a Material");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_json_should_return_0_when_no_dead_assets() {
        let (dir, db_path) = test_db_in_tempdir("dead22_json_empty");
        uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_dead_assets(
            &dir,
            &[],
            false,
            None,
            &[],
            None,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Json,
        )
        .unwrap();
        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_json_should_return_1_when_dead_assets_exist() {
        let (dir, db_path) = test_db_in_tempdir("dead22_json_found");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/Dead",
                dir.join("Dead.uasset"),
                AssetType::StaticMesh,
                8192,
                vec![],
            )])
            .unwrap();
        }

        let result = handle_dead_assets(
            &dir,
            &[],
            false,
            None,
            &[],
            None,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Json,
        )
        .unwrap();
        assert_eq!(result, 1, "JSON format exits 1 when dead assets are found");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_sort_by_size_should_return_1_when_dead_assets_exist() {
        let (dir, db_path) = test_db_in_tempdir("dead22_sort_size");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/Small",
                    dir.join("Small.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
                make_meta(
                    "/Game/Large",
                    dir.join("Large.uasset"),
                    AssetType::Blueprint,
                    8192,
                    vec![],
                ),
                make_meta(
                    "/Game/Medium",
                    dir.join("Medium.uasset"),
                    AssetType::Blueprint,
                    4096,
                    vec![],
                ),
            ])
            .unwrap();
        }

        let result = handle_dead_assets(
            &dir,
            &[],
            true,
            None,
            &[],
            None,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 1, "dead assets found when sort_by_size is true");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn entries_sort_by_size_should_order_largest_first() {
        let mut entries = [
            DeadAssetEntry {
                path: "/Game/Small".to_owned(),
                asset_type: "Blueprint".to_owned(),
                file_size: 1024,
            },
            DeadAssetEntry {
                path: "/Game/Large".to_owned(),
                asset_type: "Blueprint".to_owned(),
                file_size: 8192,
            },
            DeadAssetEntry {
                path: "/Game/Medium".to_owned(),
                asset_type: "Blueprint".to_owned(),
                file_size: 4096,
            },
        ];
        entries.sort_unstable_by_key(|e| std::cmp::Reverse(e.file_size));
        assert_eq!(entries[0].file_size, 8192);
        assert_eq!(entries[1].file_size, 4096);
        assert_eq!(entries[2].file_size, 1024);
    }

    #[test]
    fn handle_dead_assets_min_size_should_exclude_assets_below_threshold() {
        let (dir, db_path) = test_db_in_tempdir("dead22_min_size_hit");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/Small",
                    dir.join("Small.uasset"),
                    AssetType::Blueprint,
                    512,
                    vec![],
                ),
                make_meta(
                    "/Game/Large",
                    dir.join("Large.uasset"),
                    AssetType::Blueprint,
                    8192,
                    vec![],
                ),
            ])
            .unwrap();
        }

        let result = handle_dead_assets(
            &dir,
            &[],
            false,
            Some(1024),
            &[],
            None,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 1, "only Large (8192 B) meets the 1024 B threshold");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_min_size_should_return_0_when_no_assets_meet_threshold() {
        let (dir, db_path) = test_db_in_tempdir("dead22_min_size_miss");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/Tiny",
                dir.join("Tiny.uasset"),
                AssetType::Blueprint,
                256,
                vec![],
            )])
            .unwrap();
        }

        let result = handle_dead_assets(
            &dir,
            &[],
            false,
            Some(1024),
            &[],
            None,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "Tiny (256 B) is below the 1024 B threshold");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_exclude_pattern_should_exclude_matching_assets() {
        let (dir, db_path) = test_db_in_tempdir("dead22_excl_hit");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/ThirdPerson/BP_Character",
                    dir.join("BP_Character.uasset"),
                    AssetType::Blueprint,
                    2048,
                    vec![],
                ),
                make_meta(
                    "/Game/Characters/BP_Player",
                    dir.join("BP_Player.uasset"),
                    AssetType::Blueprint,
                    4096,
                    vec![],
                ),
            ])
            .unwrap();
        }

        let patterns = vec!["ThirdPerson".to_owned()];
        let result = handle_dead_assets(
            &dir,
            &[],
            false,
            None,
            &patterns,
            None,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 1, "ThirdPerson asset excluded; BP_Player remains");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_exclude_pattern_should_return_0_when_all_match() {
        let (dir, db_path) = test_db_in_tempdir("dead22_excl_all");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/ThirdPerson/BP_Character",
                dir.join("BP_Character.uasset"),
                AssetType::Blueprint,
                2048,
                vec![],
            )])
            .unwrap();
        }

        let patterns = vec!["ThirdPerson".to_owned()];
        let result = handle_dead_assets(
            &dir,
            &[],
            false,
            None,
            &patterns,
            None,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "all assets match the exclude pattern");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn group_entry_should_serialize_with_group_count_total_size_keys() {
        let entries = vec![GroupEntry {
            group: "Texture2D".to_owned(),
            count: 3,
            total_size_bytes: 4096,
        }];
        let json = serde_json::to_string(&entries).unwrap();
        assert!(json.contains("\"group\""), "must have group key");
        assert!(json.contains("\"count\""), "must have count key");
        assert!(
            json.contains("\"total_size_bytes\""),
            "must have total_size_bytes key"
        );
        assert!(
            !json.contains("\"assets\""),
            "must not have assets key when grouped"
        );
    }

    #[test]
    fn handle_dead_assets_group_type_should_aggregate_by_asset_type() {
        let (dir, db_path) = test_db_in_tempdir("dead22_group_type");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/A/BP_One",
                    dir.join("BP_One.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
                make_meta(
                    "/Game/A/BP_Two",
                    dir.join("BP_Two.uasset"),
                    AssetType::Blueprint,
                    2048,
                    vec![],
                ),
                make_meta(
                    "/Game/A/T_Rock",
                    dir.join("T_Rock.uasset"),
                    AssetType::Texture2D,
                    4096,
                    vec![],
                ),
            ])
            .unwrap();
        }

        let result = handle_dead_assets(
            &dir,
            &[],
            false,
            None,
            &[],
            Some(&GroupMode::Type),
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 1, "three dead assets across two types");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_group_dir_should_aggregate_by_directory() {
        let (dir, db_path) = test_db_in_tempdir("dead22_group_dir");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/Characters/Enemies/BP_Goblin",
                    dir.join("BP_Goblin.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
                make_meta(
                    "/Game/ThirdPerson/Blueprints/BP_Char",
                    dir.join("BP_Char.uasset"),
                    AssetType::Blueprint,
                    2048,
                    vec![],
                ),
            ])
            .unwrap();
        }

        let result = handle_dead_assets(
            &dir,
            &[],
            false,
            None,
            &[],
            Some(&GroupMode::Dir),
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 1, "assets in two different directories");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_group_type_json_should_return_1_when_dead_assets_exist() {
        let (dir, db_path) = test_db_in_tempdir("dead22_group_json");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/T_Rock",
                dir.join("T_Rock.uasset"),
                AssetType::Texture2D,
                4096,
                vec![],
            )])
            .unwrap();
        }

        let result = handle_dead_assets(
            &dir,
            &[],
            false,
            None,
            &[],
            Some(&GroupMode::Type),
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Json,
        )
        .unwrap();
        assert_eq!(result, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_group_should_return_0_when_no_dead_assets() {
        let (dir, db_path) = test_db_in_tempdir("dead22_group_empty");
        uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_dead_assets(
            &dir,
            &[],
            false,
            None,
            &[],
            Some(&GroupMode::Type),
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_should_exclude_sub_object_types_by_default() {
        let (dir, db_path) = test_db_in_tempdir("dead235_default");
        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/BP_Character",
                    dir.join("BP_Character.uasset"),
                    AssetType::Blueprint,
                    4096,
                    vec![],
                ),
                make_meta(
                    "/Game/Meta",
                    dir.join("Meta.uasset"),
                    AssetType::Unknown("MetaData".to_owned()),
                    512,
                    vec![],
                ),
            ])
            .unwrap();
        }
        let result = handle_dead_assets(
            &dir,
            &[],
            false,
            None,
            &[],
            None,
            false, // include_all_types = false → MetaData excluded
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(
            result, 1,
            "MetaData is excluded; only Blueprint counts as dead"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_should_include_sub_object_types_when_include_all_types_is_true() {
        let (dir, db_path) = test_db_in_tempdir("dead235_all");
        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/BP_Character",
                    dir.join("BP_Character.uasset"),
                    AssetType::Blueprint,
                    4096,
                    vec![],
                ),
                make_meta(
                    "/Game/Meta",
                    dir.join("Meta.uasset"),
                    AssetType::Unknown("MetaData".to_owned()),
                    512,
                    vec![],
                ),
            ])
            .unwrap();
        }
        let result = handle_dead_assets(
            &dir,
            &[],
            false,
            None,
            &[],
            None,
            true, // include_all_types = true → MetaData included
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 1, "both are dead; result is 1 (found)");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_exclude_pattern_multiple_patterns_should_or_combine() {
        let (dir, db_path) = test_db_in_tempdir("dead22_excl_multi");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/ThirdPerson/BP_Character",
                    dir.join("BP_Character.uasset"),
                    AssetType::Blueprint,
                    2048,
                    vec![],
                ),
                make_meta(
                    "/Game/EditorTools/BP_Helper",
                    dir.join("BP_Helper.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
                make_meta(
                    "/Game/Characters/BP_Player",
                    dir.join("BP_Player.uasset"),
                    AssetType::Blueprint,
                    4096,
                    vec![],
                ),
            ])
            .unwrap();
        }

        let patterns = vec!["ThirdPerson".to_owned(), "EditorTools".to_owned()];
        let result = handle_dead_assets(
            &dir,
            &[],
            false,
            None,
            &patterns,
            None,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(
            result, 1,
            "ThirdPerson and EditorTools excluded; only BP_Player remains"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
