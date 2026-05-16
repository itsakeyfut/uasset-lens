use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;

use crate::FormatKind;

#[derive(serde::Serialize)]
struct DeadAssetEntry {
    path: String,
    #[serde(rename = "type")]
    asset_type: String,
    file_size: u64,
}

#[derive(serde::Serialize)]
struct DeadAssetsOutput {
    assets: Vec<DeadAssetEntry>,
    total_size_bytes: u64,
}

pub fn handle_dead_assets(
    _project_dir: &Path,
    asset_type_filter: Option<&str>,
    sort_by_size: bool,
    db_path: &Path,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    let db = crate::open_db(db_path)?;
    let graph = crate::load_graph(&db)?;

    let dead_paths = dead_asset_detector::detect(&graph);

    let type_map: HashMap<&shared::AssetPath, String> = graph
        .nodes()
        .map(|n| (&n.path, n.asset_type.to_string()))
        .collect();

    let mut entries: Vec<DeadAssetEntry> = dead_paths
        .iter()
        .filter(|path| {
            asset_type_filter
                .map(|filter| type_map.get(path).map(|t| t == filter).unwrap_or(false))
                .unwrap_or(true)
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
                path: path.as_str().to_owned(), // clone required: AssetPath is not Copy
                asset_type,
                file_size,
            }
        })
        .collect();

    if sort_by_size {
        entries.sort_unstable_by_key(|e| std::cmp::Reverse(e.file_size));
    }

    let count = entries.len();
    let total_size_bytes: u64 = entries.iter().map(|e| e.file_size).sum();

    match format {
        FormatKind::Json => {
            let output = DeadAssetsOutput {
                assets: entries,
                total_size_bytes,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&output)
                    .context("Failed to serialize dead assets output to JSON")?
            );
        }
        FormatKind::Text => {
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
        }
    }

    if count == 0 { Ok(0) } else { Ok(1) }
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
    use crate::commands::make_meta;
    use shared::{AssetPath, AssetType};

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
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"assets\""), "JSON must contain assets key");
        assert!(json.contains("/Game/A"));
    }

    #[test]
    fn handle_dead_assets_should_return_err_when_db_does_not_exist() {
        let db_path = std::env::temp_dir().join(format!(
            "uasset_lens_dead22_missing_{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);

        let result =
            handle_dead_assets(Path::new("/proj"), None, false, &db_path, &FormatKind::Text);
        assert!(result.is_err(), "missing DB should return an error");
    }

    #[test]
    fn handle_dead_assets_should_return_0_when_db_has_no_assets() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_dead22_empty_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_dead_assets(&dir, None, false, &db_path, &FormatKind::Text).unwrap();
        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_should_return_0_when_all_nodes_form_cycle() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_dead22_cycle_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
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

        let result = handle_dead_assets(&dir, None, false, &db_path, &FormatKind::Text).unwrap();
        assert_eq!(result, 0, "all nodes in a cycle have in_degree >= 1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_should_return_1_when_unreferenced_asset_exists() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_dead22_found_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/Orphan",
                dir.join("Orphan.uasset"),
                AssetType::Blueprint,
                4096,
                vec![],
            )])
            .unwrap();
        }

        let result = handle_dead_assets(&dir, None, false, &db_path, &FormatKind::Text).unwrap();
        assert_eq!(result, 1, "/Game/Orphan has no incoming edges");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_type_filter_should_return_0_when_no_type_match() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_dead22_filter_miss_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/OrphanBP",
                dir.join("OrphanBP.uasset"),
                AssetType::Blueprint,
                1024,
                vec![],
            )])
            .unwrap();
        }

        let result =
            handle_dead_assets(&dir, Some("Texture2D"), false, &db_path, &FormatKind::Text)
                .unwrap();
        assert_eq!(
            result, 0,
            "dead asset is Blueprint, filter is Texture2D — no match"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_type_filter_should_return_1_when_type_matches() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_dead22_filter_hit_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
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

        let result =
            handle_dead_assets(&dir, Some("Texture2D"), false, &db_path, &FormatKind::Text)
                .unwrap();
        assert_eq!(result, 1, "Texture2D dead asset matches the filter");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_json_should_return_0_when_no_dead_assets() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_dead22_json_empty_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_dead_assets(&dir, None, false, &db_path, &FormatKind::Json).unwrap();
        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_json_should_return_1_when_dead_assets_exist() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_dead22_json_found_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/Dead",
                dir.join("Dead.uasset"),
                AssetType::StaticMesh,
                8192,
                vec![],
            )])
            .unwrap();
        }

        let result = handle_dead_assets(&dir, None, false, &db_path, &FormatKind::Json).unwrap();
        assert_eq!(result, 1, "JSON format exits 1 when dead assets are found");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_dead_assets_sort_by_size_should_return_1_when_dead_assets_exist() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_dead22_sort_size_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
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

        let result = handle_dead_assets(&dir, None, true, &db_path, &FormatKind::Text).unwrap();
        assert_eq!(result, 1, "dead assets found when sort_by_size is true");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn entries_sort_by_size_should_order_largest_first() {
        let mut entries = vec![
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
}
