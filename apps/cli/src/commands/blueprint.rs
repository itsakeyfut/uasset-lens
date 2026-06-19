use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;

use crate::FormatKind;

#[derive(serde::Serialize)]
struct BlueprintEntry {
    asset_path: String,
    node_count: u32,
    event_tick_count: u32,
    cast_count: u32,
    dependency_depth: u32,
}

pub fn handle_blueprint(
    project_dir: &Path,
    db_path: &Path,
    format: &FormatKind,
    quiet: bool,
) -> anyhow::Result<i32> {
    crate::maybe_hint_github_actions(format, quiet);
    let db = crate::open_db(db_path)?;

    let mut rows = db
        .all_blueprint_metrics()
        .context("Failed to read blueprint metrics from database")?;

    rows.sort_by_key(|r| std::cmp::Reverse(r.node_count));

    let entries: Vec<BlueprintEntry> = rows
        .iter()
        .map(|r| BlueprintEntry {
            asset_path: r.asset_path.as_str().to_owned(),
            node_count: r.node_count,
            event_tick_count: r.event_tick_count,
            cast_count: r.cast_count,
            dependency_depth: r.dependency_depth,
        })
        .collect();

    match format {
        FormatKind::Sarif => return Err(crate::sarif_not_supported()),
        FormatKind::GithubActions => {
            let assets = db
                .all_assets()
                .context("Failed to read assets from database")?;
            let path_lookup: HashMap<&str, &std::path::Path> = assets
                .iter()
                .map(|r| (r.asset_path.as_str(), r.file_path.as_path()))
                .collect();
            for e in &entries {
                let rel = crate::rel_path_for_annotation(
                    path_lookup
                        .get(e.asset_path.as_str())
                        .copied()
                        .unwrap_or(std::path::Path::new("")),
                    project_dir,
                );
                let msg = format!(
                    "nodes={}, ticks={}, casts={}, depth={}",
                    e.node_count, e.event_tick_count, e.cast_count, e.dependency_depth
                );
                println!(
                    "{}",
                    crate::format_gh_annotation("notice", &rel, "BlueprintComplexity", &msg)
                );
            }
            return Ok(0);
        }
        FormatKind::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&entries)
                    .context("Failed to serialize blueprint output to JSON")?
            );
        }
        FormatKind::Text => {
            println!("Blueprint Complexity Report");
            println!("===========================");
            if entries.is_empty() {
                println!("  (no Blueprint assets found)");
            } else {
                println!(
                    "{:>4}  {:<40}  {:>6}  {:>6}  {:>6}  {:>6}",
                    "Rank", "Asset", "Nodes", "Ticks", "Casts", "Depth"
                );
                for (i, e) in entries.iter().enumerate() {
                    println!(
                        "{:>4}  {:<40}  {:>6}  {:>6}  {:>6}  {:>6}",
                        i + 1,
                        truncate_path(&e.asset_path, 40),
                        e.node_count,
                        e.event_tick_count,
                        e.cast_count,
                        e.dependency_depth,
                    );
                }
                println!();
                println!("  {} Blueprint asset(s) ranked", entries.len());
            }
        }
    }

    Ok(0)
}

fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        path.to_owned()
    } else {
        format!("...{}", &path[path.len() - (max_len - 3)..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_db_in_tempdir;
    use std::path::PathBuf;
    use uasset_lens_shared::{AssetPath, AssetType};

    fn make_db_with_blueprints(path: &std::path::Path) -> uasset_lens_asset_db::AssetDb {
        let mut db = uasset_lens_asset_db::AssetDb::open(path).unwrap();
        let assets = vec![
            uasset_lens_scanner::AssetMetadata {
                asset_path: AssetPath::new("/Game/BP_Boss").unwrap(),
                file_path: PathBuf::from("/proj/Content/BP_Boss.uasset"),
                asset_type: AssetType::Blueprint,
                file_size: 4096,
                last_modified: 100,
                dependencies: vec![],
                soft_dependencies: vec![],
                blueprint_metrics: Some(uasset_lens_scanner::BlueprintMetrics {
                    node_count: 412,
                    event_tick_count: 3,
                    cast_count: 24,
                    dependency_depth: 4,
                }),
                material_texture_samples: None,
            },
            uasset_lens_scanner::AssetMetadata {
                asset_path: AssetPath::new("/Game/BP_Player").unwrap(),
                file_path: PathBuf::from("/proj/Content/BP_Player.uasset"),
                asset_type: AssetType::Blueprint,
                file_size: 2048,
                last_modified: 100,
                dependencies: vec![],
                soft_dependencies: vec![],
                blueprint_metrics: Some(uasset_lens_scanner::BlueprintMetrics {
                    node_count: 312,
                    event_tick_count: 1,
                    cast_count: 18,
                    dependency_depth: 3,
                }),
                material_texture_samples: None,
            },
        ];
        db.upsert_all(&assets).unwrap();
        db
    }

    #[test]
    fn handle_blueprint_github_actions_should_return_0() {
        let (dir, db_path) = test_db_in_tempdir("bp_ga");

        make_db_with_blueprints(&db_path);

        let result = handle_blueprint(&dir, &db_path, &FormatKind::GithubActions, false).unwrap();

        assert_eq!(result, 0, "blueprint is informational — always exits 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_blueprint_should_return_err_when_db_does_not_exist() {
        let (dir, db_path) = test_db_in_tempdir("bp_missing");
        let _ = std::fs::remove_dir_all(&dir);
        let result = handle_blueprint(
            std::path::Path::new("."),
            &db_path,
            &FormatKind::Text,
            false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn handle_blueprint_should_return_0_when_no_blueprint_assets() {
        let (dir, db_path) = test_db_in_tempdir("bp_empty");
        uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_blueprint(&dir, &db_path, &FormatKind::Text, false).unwrap();

        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_blueprint_should_return_0_with_blueprint_assets() {
        let (dir, db_path) = test_db_in_tempdir("bp_json");

        make_db_with_blueprints(&db_path);

        let result = handle_blueprint(&dir, &db_path, &FormatKind::Json, false).unwrap();

        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn blueprint_rows_should_be_sorted_by_node_count_descending() {
        let (dir, db_path) = test_db_in_tempdir("bp_sort");

        let db = make_db_with_blueprints(&db_path);

        let mut rows = db.all_blueprint_metrics().unwrap();
        rows.sort_by_key(|r| std::cmp::Reverse(r.node_count));

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].asset_path.as_str(), "/Game/BP_Boss");
        assert_eq!(rows[0].node_count, 412);
        assert_eq!(rows[1].asset_path.as_str(), "/Game/BP_Player");
        assert_eq!(rows[1].node_count, 312);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
