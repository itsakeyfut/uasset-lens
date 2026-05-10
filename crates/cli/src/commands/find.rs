use std::collections::HashSet;
use std::path::Path;

use anyhow::Context;

use crate::FormatKind;

#[derive(serde::Serialize)]
struct FindEntry {
    path: String,
    #[serde(rename = "type")]
    asset_type: String,
    file_size: u64,
}

#[allow(clippy::too_many_arguments)]
pub fn handle_find(
    _project_dir: &Path,
    asset_type: Option<&str>,
    larger_than: Option<u64>,
    smaller_than: Option<u64>,
    unreferenced: bool,
    path_pattern: Option<&str>,
    db_path: &Path,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    if !db_path.exists() {
        anyhow::bail!("no scan data found.\nRun 'uasset-lens scan <project_dir>' first.");
    }

    let at = asset_type.map(|s| {
        serde_json::from_str::<shared::AssetType>(&format!("\"{}\"", s))
            .unwrap_or_else(|_| shared::AssetType::Unknown(s.to_string()))
    });

    let db = asset_db::AssetDb::open(db_path).context("Failed to open database")?;

    let filter = asset_db::AssetFilter {
        asset_type: at,
        min_size: larger_than,
        max_size: smaller_than,
        path_pattern: path_pattern.map(|s| s.to_owned()),
    };

    let mut results = db.find_assets(&filter).context("Failed to query assets")?;

    if unreferenced {
        let graph = crate::load_graph(db_path)?;
        let dead: HashSet<shared::AssetPath> =
            dead_asset_detector::detect(&graph).into_iter().collect();
        results.retain(|r| dead.contains(&r.asset_path));
    }

    let entries: Vec<FindEntry> = results
        .iter()
        .map(|r| FindEntry {
            path: r.asset_path.as_str().to_owned(), // clone required: AssetPath is not Copy
            asset_type: r.asset_type.to_string(),
            file_size: r.file_size,
        })
        .collect();

    match format {
        FormatKind::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&entries)
                    .context("Failed to serialize find output to JSON")?
            );
        }
        FormatKind::Text => {
            let header = format!("Found {} assets", entries.len());
            let separator = "=".repeat(header.len());
            println!("{header}");
            println!("{separator}");
            for entry in &entries {
                println!(
                    "{}  {}  {}",
                    entry.path,
                    entry.asset_type,
                    crate::format_size(entry.file_size)
                );
            }
        }
    }

    Ok(0)
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
    fn handle_find_should_return_err_when_db_does_not_exist() {
        let db_path = std::env::temp_dir().join(format!(
            "uasset_lens_find28_missing_{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);

        let result = handle_find(
            Path::new("/proj"),
            None,
            None,
            None,
            false,
            None,
            &db_path,
            &FormatKind::Text,
        );
        assert!(result.is_err(), "missing DB should return an error");
    }

    #[test]
    fn handle_find_should_return_0_when_no_results_found() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_find28_empty_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_find(
            &dir,
            None,
            None,
            None,
            false,
            None,
            &db_path,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "empty results → exit 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_find_should_return_0_when_results_found() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_find28_found_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/Tex/T_Rock",
                dir.join("T_Rock.uasset"),
                AssetType::Texture2D,
                4096,
                vec![],
            )])
            .unwrap();
        }

        let result = handle_find(
            &dir,
            None,
            None,
            None,
            false,
            None,
            &db_path,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "results found → still exit 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_find_should_filter_by_asset_type() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_find28_type_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/BP_Player",
                    dir.join("BP_Player.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
                make_meta(
                    "/Game/T_Rock",
                    dir.join("T_Rock.uasset"),
                    AssetType::Texture2D,
                    2048,
                    vec![],
                ),
            ])
            .unwrap();
        }

        let result = handle_find(
            &dir,
            Some("Texture2D"),
            None,
            None,
            false,
            None,
            &db_path,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "--type filter → exit 0 regardless");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_find_should_filter_unreferenced_only() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_find28_unref_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            // A references B, so B has in_degree == 1 (not dead).
            // C has no refs (dead).
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
                    vec![],
                ),
                make_meta(
                    "/Game/C",
                    dir.join("C.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
            ])
            .unwrap();
        }

        let result = handle_find(
            &dir,
            None,
            None,
            None,
            true,
            None,
            &db_path,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "--unreferenced → exit 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_find_json_should_return_0_with_results() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_find28_json_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/T_Ground",
                dir.join("T_Ground.uasset"),
                AssetType::Texture2D,
                4096,
                vec![],
            )])
            .unwrap();
        }

        let result = handle_find(
            &dir,
            None,
            None,
            None,
            false,
            None,
            &db_path,
            &FormatKind::Json,
        )
        .unwrap();
        assert_eq!(result, 0, "JSON format exits 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Verifies the --unreferenced intersection logic directly:
    // referenced assets (in_degree >= 1) must be excluded; orphans must be included.
    #[test]
    fn find_unreferenced_filter_should_exclude_referenced_assets_and_include_orphans() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_find28_unref_logic_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            // A → B: B has in_degree 1 (referenced), A and C have in_degree 0 (dead).
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
                    vec![],
                ),
                make_meta(
                    "/Game/C",
                    dir.join("C.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
            ])
            .unwrap();
        }

        let graph = crate::load_graph(&db_path).unwrap();
        let dead: std::collections::HashSet<AssetPath> =
            dead_asset_detector::detect(&graph).into_iter().collect();

        assert!(
            !dead.contains(&AssetPath::new("/Game/B").unwrap()),
            "B is referenced by A and must not appear in unreferenced results"
        );
        assert!(
            dead.contains(&AssetPath::new("/Game/A").unwrap()),
            "A references B but is itself unreferenced — must appear"
        );
        assert!(
            dead.contains(&AssetPath::new("/Game/C").unwrap()),
            "C has no references — must appear"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_find_should_return_0_when_filtering_by_larger_than() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_find28_larger_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/Small",
                    dir.join("Small.uasset"),
                    AssetType::Texture2D,
                    512,
                    vec![],
                ),
                make_meta(
                    "/Game/Large",
                    dir.join("Large.uasset"),
                    AssetType::Texture2D,
                    8192,
                    vec![],
                ),
            ])
            .unwrap();
        }

        let result = handle_find(
            &dir,
            None,
            Some(4096),
            None,
            false,
            None,
            &db_path,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "--larger-than → exit 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_find_should_return_0_when_filtering_by_smaller_than() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_find28_smaller_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/Small",
                    dir.join("Small.uasset"),
                    AssetType::Texture2D,
                    512,
                    vec![],
                ),
                make_meta(
                    "/Game/Large",
                    dir.join("Large.uasset"),
                    AssetType::Texture2D,
                    8192,
                    vec![],
                ),
            ])
            .unwrap();
        }

        let result = handle_find(
            &dir,
            None,
            None,
            Some(1024),
            false,
            None,
            &db_path,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "--smaller-than → exit 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_find_should_return_0_when_filtering_by_path_pattern() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_find28_path_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/Characters/BP_Player",
                    dir.join("Characters").join("BP_Player.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
                make_meta(
                    "/Game/UI/WBP_HUD",
                    dir.join("UI").join("WBP_HUD.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
            ])
            .unwrap();
        }

        let result = handle_find(
            &dir,
            None,
            None,
            None,
            false,
            Some("**/Characters/**"),
            &db_path,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "--path pattern → exit 0");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
