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

pub fn handle_dead_assets(
    _project_dir: &Path,
    asset_type_filter: Option<&str>,
    db_path: &Path,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    let graph = crate::load_graph(db_path)?;
    let db = asset_db::AssetDb::open(db_path).context("Failed to open database")?;

    let dead_paths = dead_asset_detector::detect(&graph);

    let type_map: HashMap<&shared::AssetPath, String> = graph
        .nodes()
        .map(|n| (&n.path, n.asset_type.to_string()))
        .collect();

    let entries: Vec<DeadAssetEntry> = dead_paths
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

    match format {
        FormatKind::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&entries)
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
            if !entries.is_empty() {
                println!();
            }
            println!("  Dead Assets ({} found)", entries.len());
        }
    }

    if entries.is_empty() { Ok(0) } else { Ok(1) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::{AssetPath, AssetType};

    fn make_meta(
        asset_path: &str,
        file_path: std::path::PathBuf,
        asset_type: AssetType,
        file_size: u64,
        deps: Vec<AssetPath>,
    ) -> scanner::AssetMetadata {
        scanner::AssetMetadata {
            asset_path: AssetPath::new(asset_path).unwrap(),
            file_path,
            asset_type,
            file_size,
            last_modified: 0,
            dependencies: deps,
        }
    }

    #[test]
    fn handle_dead_assets_should_return_err_when_db_does_not_exist() {
        let db_path = std::env::temp_dir().join(format!(
            "uasset_lens_dead22_missing_{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);

        let result = handle_dead_assets(Path::new("/proj"), None, &db_path, &FormatKind::Text);
        assert!(result.is_err(), "missing DB should return an error");
    }

    #[test]
    fn handle_dead_assets_should_return_0_when_db_has_no_assets() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_dead22_empty_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_dead_assets(&dir, None, &db_path, &FormatKind::Text).unwrap();
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

        let result = handle_dead_assets(&dir, None, &db_path, &FormatKind::Text).unwrap();
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

        let result = handle_dead_assets(&dir, None, &db_path, &FormatKind::Text).unwrap();
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
            handle_dead_assets(&dir, Some("Texture2D"), &db_path, &FormatKind::Text).unwrap();
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
            handle_dead_assets(&dir, Some("Texture2D"), &db_path, &FormatKind::Text).unwrap();
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

        let result = handle_dead_assets(&dir, None, &db_path, &FormatKind::Json).unwrap();
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

        let result = handle_dead_assets(&dir, None, &db_path, &FormatKind::Json).unwrap();
        assert_eq!(result, 1, "JSON format exits 1 when dead assets are found");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
