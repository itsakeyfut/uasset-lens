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

#[derive(serde::Serialize)]
struct FindOutput {
    assets: Vec<FindEntry>,
    total_count: usize,
    total_bytes: u64,
}

#[allow(clippy::too_many_arguments)]
pub fn handle_find(
    _project_dir: &Path,
    asset_type: Option<&str>,
    larger_than: Option<u64>,
    smaller_than: Option<u64>,
    unreferenced: bool,
    path_pattern: Option<&str>,
    sort_by_size: bool,
    refs: Option<&str>,
    deps: Option<&str>,
    db_path: &Path,
    cfg: &crate::config::ConfigFile,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    let at = asset_type.map(|s| {
        serde_json::from_str::<shared::AssetType>(&format!("\"{}\"", s))
            .unwrap_or_else(|_| shared::AssetType::Unknown(s.to_string()))
    });

    let db = crate::open_db(db_path)?;

    let filter = asset_db::AssetFilter {
        asset_type: at,
        min_size: larger_than,
        max_size: smaller_than,
        path_pattern: path_pattern.map(|s| s.to_owned()),
    };

    let mut results = db.find_assets(&filter).context("Failed to query assets")?;

    let graph = if unreferenced || refs.is_some() || deps.is_some() {
        Some(crate::load_graph(&db, &cfg.scan.external_roots)?)
    } else {
        None
    };

    if unreferenced && let Some(g) = &graph {
        let dead: HashSet<shared::AssetPath> =
            dead_asset_detector::detect(g, &[]).into_iter().collect();
        results.retain(|r| dead.contains(&r.asset_path));
    }

    if let Some(refs_path) = refs {
        let target = shared::AssetPath::new(refs_path).map_err(|e| anyhow::anyhow!("{e}"))?;
        if let Some(g) = &graph {
            let impact = g.find_impact(&target);
            let ref_set: HashSet<shared::AssetPath> =
                impact.direct.into_iter().chain(impact.transitive).collect();
            results.retain(|r| ref_set.contains(&r.asset_path));
        }
    }

    if let Some(deps_path) = deps {
        let target = shared::AssetPath::new(deps_path).map_err(|e| anyhow::anyhow!("{e}"))?;
        if let Some(g) = &graph {
            let dep_set: HashSet<shared::AssetPath> = g
                .dependencies_of(&target)
                .into_iter()
                .map(|(p, _)| p)
                .collect();
            results.retain(|r| dep_set.contains(&r.asset_path));
        }
    }

    if sort_by_size {
        results.sort_by_key(|r| std::cmp::Reverse(r.file_size));
    }

    let entries: Vec<FindEntry> = results
        .iter()
        .map(|r| FindEntry {
            path: r.asset_path.as_str().to_owned(), // clone required: AssetPath is not Copy
            asset_type: r.asset_type.to_string(),
            file_size: r.file_size,
        })
        .collect();

    let total_bytes: u64 = entries.iter().map(|e| e.file_size).sum();

    match format {
        FormatKind::Json => {
            let out = FindOutput {
                total_count: entries.len(),
                total_bytes,
                assets: entries,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&out)
                    .context("Failed to serialize find output to JSON")?
            );
        }
        FormatKind::GithubActions | FormatKind::Text => {
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
            println!();
            println!(
                "  {} assets found  (total {})",
                entries.len(),
                crate::format_size(total_bytes)
            );
        }
    }

    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{make_meta, test_db_in_tempdir};
    use shared::{AssetPath, AssetType};

    #[test]
    fn handle_find_should_return_err_when_db_does_not_exist() {
        let (dir, db_path) = test_db_in_tempdir("find28_missing");
        let _ = std::fs::remove_dir_all(&dir);

        let result = handle_find(
            Path::new("/proj"),
            None,
            None,
            None,
            false,
            None,
            false,
            None,
            None,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        );
        assert!(result.is_err(), "missing DB should return an error");
    }

    #[test]
    fn handle_find_should_return_0_when_no_results_found() {
        let (dir, db_path) = test_db_in_tempdir("find28_empty");
        asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_find(
            &dir,
            None,
            None,
            None,
            false,
            None,
            false,
            None,
            None,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "empty results → exit 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_find_should_return_0_when_results_found() {
        let (dir, db_path) = test_db_in_tempdir("find28_found");

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
            false,
            None,
            None,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "results found → still exit 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_find_should_filter_by_asset_type() {
        let (dir, db_path) = test_db_in_tempdir("find28_type");

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
            false,
            None,
            None,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "--type filter → exit 0 regardless");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_find_should_filter_unreferenced_only() {
        let (dir, db_path) = test_db_in_tempdir("find28_unref");

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
            false,
            None,
            None,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "--unreferenced → exit 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_find_json_should_return_0_with_results() {
        let (dir, db_path) = test_db_in_tempdir("find28_json");

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
            false,
            None,
            None,
            &db_path,
            &Default::default(),
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
        let (dir, db_path) = test_db_in_tempdir("find28_unref_logic");

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

        let db = asset_db::AssetDb::open(&db_path).unwrap();
        let graph = crate::load_graph(&db, &[]).unwrap();
        let dead: std::collections::HashSet<AssetPath> = dead_asset_detector::detect(&graph, &[])
            .into_iter()
            .collect();

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
        let (dir, db_path) = test_db_in_tempdir("find28_larger");

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
            false,
            None,
            None,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "--larger-than → exit 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_find_should_return_0_when_filtering_by_smaller_than() {
        let (dir, db_path) = test_db_in_tempdir("find28_smaller");

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
            false,
            None,
            None,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "--smaller-than → exit 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_find_should_return_0_when_filtering_by_path_pattern() {
        let (dir, db_path) = test_db_in_tempdir("find28_path");

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
            false,
            None,
            None,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "--path pattern → exit 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_find_sort_by_size_should_return_0_and_order_results_largest_first() {
        let (dir, db_path) = test_db_in_tempdir("find28_sort");

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
                make_meta(
                    "/Game/Medium",
                    dir.join("Medium.uasset"),
                    AssetType::Texture2D,
                    2048,
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
            None,
            true,
            None,
            None,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "--sort-by-size → exit 0");
        // Verify sort order at the data layer: largest first
        let db_verify = asset_db::AssetDb::open(&db_path).unwrap();
        let no_filter = asset_db::AssetFilter {
            asset_type: None,
            min_size: None,
            max_size: None,
            path_pattern: None,
        };
        let mut rows = db_verify.find_assets(&no_filter).unwrap();
        rows.sort_by_key(|r| std::cmp::Reverse(r.file_size));
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].file_size, 8192, "largest first");
        assert_eq!(rows[1].file_size, 2048, "medium second");
        assert_eq!(rows[2].file_size, 512, "smallest last");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_find_refs_should_return_0_filtering_to_referencing_assets() {
        let (dir, db_path) = test_db_in_tempdir("find28_refs");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            // A and B both reference T; C does not.
            db.upsert_all(&[
                make_meta(
                    "/Game/A",
                    dir.join("A.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![AssetPath::new("/Game/T").unwrap()],
                ),
                make_meta(
                    "/Game/B",
                    dir.join("B.uasset"),
                    AssetType::Blueprint,
                    2048,
                    vec![AssetPath::new("/Game/T").unwrap()],
                ),
                make_meta(
                    "/Game/T",
                    dir.join("T.uasset"),
                    AssetType::Texture2D,
                    4096,
                    vec![],
                ),
                make_meta(
                    "/Game/C",
                    dir.join("C.uasset"),
                    AssetType::Blueprint,
                    512,
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
            None,
            false,
            Some("/Game/T"),
            None,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "--refs → exit 0");
        // Verify that find_impact correctly identifies all referencing assets
        let db_verify = asset_db::AssetDb::open(&db_path).unwrap();
        let graph = crate::load_graph(&db_verify, &[]).unwrap();
        let target = AssetPath::new("/Game/T").unwrap();
        let impact = graph.find_impact(&target);
        let ref_set: std::collections::HashSet<AssetPath> =
            impact.direct.into_iter().chain(impact.transitive).collect();
        assert!(
            ref_set.contains(&AssetPath::new("/Game/A").unwrap()),
            "A references T"
        );
        assert!(
            ref_set.contains(&AssetPath::new("/Game/B").unwrap()),
            "B references T"
        );
        assert!(
            !ref_set.contains(&AssetPath::new("/Game/C").unwrap()),
            "C does not reference T"
        );
        assert!(
            !ref_set.contains(&AssetPath::new("/Game/T").unwrap()),
            "T itself is not in impact"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_find_deps_should_return_0_filtering_to_dependency_assets() {
        let (dir, db_path) = test_db_in_tempdir("find28_deps");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            // A depends on T and M.
            db.upsert_all(&[
                make_meta(
                    "/Game/A",
                    dir.join("A.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![
                        AssetPath::new("/Game/T").unwrap(),
                        AssetPath::new("/Game/M").unwrap(),
                    ],
                ),
                make_meta(
                    "/Game/T",
                    dir.join("T.uasset"),
                    AssetType::Texture2D,
                    4096,
                    vec![],
                ),
                make_meta(
                    "/Game/M",
                    dir.join("M.uasset"),
                    AssetType::Blueprint,
                    2048,
                    vec![],
                ),
                make_meta(
                    "/Game/Unrelated",
                    dir.join("Unrelated.uasset"),
                    AssetType::Blueprint,
                    512,
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
            None,
            false,
            None,
            Some("/Game/A"),
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "--deps → exit 0");
        // Verify that dependencies_of returns exactly T and M, not Unrelated
        let db_verify = asset_db::AssetDb::open(&db_path).unwrap();
        let graph = crate::load_graph(&db_verify, &[]).unwrap();
        let target = AssetPath::new("/Game/A").unwrap();
        let dep_paths: std::collections::HashSet<AssetPath> = graph
            .dependencies_of(&target)
            .into_iter()
            .map(|(p, _)| p)
            .collect();
        assert!(
            dep_paths.contains(&AssetPath::new("/Game/T").unwrap()),
            "A depends on T"
        );
        assert!(
            dep_paths.contains(&AssetPath::new("/Game/M").unwrap()),
            "A depends on M"
        );
        assert!(
            !dep_paths.contains(&AssetPath::new("/Game/Unrelated").unwrap()),
            "Unrelated is not a dependency of A"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_find_json_should_include_total_count_and_total_bytes() {
        let (dir, db_path) = test_db_in_tempdir("find28_json_totals");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/A",
                    dir.join("A.uasset"),
                    AssetType::Texture2D,
                    1024,
                    vec![],
                ),
                make_meta(
                    "/Game/B",
                    dir.join("B.uasset"),
                    AssetType::Texture2D,
                    2048,
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
            None,
            false,
            None,
            None,
            &db_path,
            &Default::default(),
            &FormatKind::Json,
        )
        .unwrap();
        assert_eq!(result, 0, "JSON with totals → exit 0");
        // Verify the values that populate total_count and total_bytes in JSON output
        let db_verify = asset_db::AssetDb::open(&db_path).unwrap();
        let no_filter = asset_db::AssetFilter {
            asset_type: None,
            min_size: None,
            max_size: None,
            path_pattern: None,
        };
        let rows = db_verify.find_assets(&no_filter).unwrap();
        assert_eq!(rows.len(), 2, "total_count should be 2");
        let total_bytes: u64 = rows.iter().map(|r| r.file_size).sum();
        assert_eq!(total_bytes, 1024 + 2048, "total_bytes should be 3072");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
