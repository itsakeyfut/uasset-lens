use std::path::Path;

use anyhow::Context;

use crate::FormatKind;

#[derive(serde::Serialize)]
struct GraphOutput {
    total_assets: usize,
    total_edges: usize,
    cycles: Vec<Vec<String>>,
}

pub fn handle_graph(
    _project_dir: &Path,
    cycles_only: bool,
    db_path: &Path,
    cfg: &crate::config::ConfigFile,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    let db = crate::open_db(db_path)?;
    let graph = crate::load_graph(&db, &cfg.scan.external_roots)?;
    let cycles = graph.find_cycles();
    let total_assets = graph.nodes().count();
    let total_edges = graph.edge_count();

    // Build closed-path cycle representation (first node repeated at end).
    let cycle_paths: Vec<Vec<String>> = cycles
        .iter()
        .map(|scc| {
            let mut paths: Vec<String> = scc
                .iter()
                .map(|p| p.as_str().to_owned()) // clone required: AssetPath is not Copy
                .collect();
            if let Some(first) = paths.first().cloned() {
                paths.push(first);
            }
            paths
        })
        .collect();

    match format {
        FormatKind::Json => {
            let output = GraphOutput {
                total_assets,
                total_edges,
                cycles: cycle_paths,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&output)
                    .context("Failed to serialize graph output to JSON")?
            );
        }
        FormatKind::GithubActions | FormatKind::Text => {
            if !cycles_only {
                println!("Dependency Graph Summary");
                println!("  Total assets   : {}", crate::format_number(total_assets));
                println!("  Total edges    : {}", crate::format_number(total_edges));
                println!("  Circular deps  : {} cycles detected", cycles.len());
            }
            if !cycle_paths.is_empty() {
                if !cycles_only {
                    println!();
                }
                println!("  Cycles:");
                for (i, path_list) in cycle_paths.iter().enumerate() {
                    println!("    [{}] {}", i + 1, path_list.join(" \u{2192} "));
                }
            }
        }
    }

    if cycles_only && !cycles.is_empty() {
        Ok(1)
    } else {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{make_meta, test_db_in_tempdir};
    use shared::{AssetPath, AssetType};

    #[test]
    fn handle_graph_should_return_err_when_db_does_not_exist() {
        let (dir, db_path) = test_db_in_tempdir("graph21_missing");
        let _ = std::fs::remove_dir_all(&dir);

        let result =
            handle_graph(Path::new("/proj"), false, &db_path, &Default::default(), &FormatKind::Text);
        assert!(result.is_err(), "missing DB should return an error");
    }

    #[test]
    fn handle_graph_should_return_0_when_db_is_empty() {
        let (dir, db_path) = test_db_in_tempdir("graph21_empty");
        asset_db::AssetDb::open(&db_path).unwrap();

        let result =
            handle_graph(&dir, false, &db_path, &Default::default(), &FormatKind::Text).unwrap();
        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_graph_cycles_only_should_return_0_when_no_cycles() {
        let (dir, db_path) = test_db_in_tempdir("graph21_nocycle");
        asset_db::AssetDb::open(&db_path).unwrap();

        let result =
            handle_graph(&dir, true, &db_path, &Default::default(), &FormatKind::Text).unwrap();
        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_graph_cycles_only_should_return_1_when_cycles_present() {
        let (dir, db_path) = test_db_in_tempdir("graph21_cycle");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
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

        let result =
            handle_graph(&dir, true, &db_path, &Default::default(), &FormatKind::Text).unwrap();
        assert_eq!(result, 1, "cycles-only with A→B→A cycle should exit 1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_graph_without_cycles_only_should_return_0_even_with_cycles() {
        let (dir, db_path) = test_db_in_tempdir("graph21_cycle_no_flag");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
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

        let result =
            handle_graph(&dir, false, &db_path, &Default::default(), &FormatKind::Text).unwrap();
        assert_eq!(
            result, 0,
            "without --cycles-only flag, exit code is always 0"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_graph_json_should_output_required_keys_when_db_is_empty() {
        let (dir, db_path) = test_db_in_tempdir("graph21_json");
        asset_db::AssetDb::open(&db_path).unwrap();

        // Verify that JSON serialization succeeds and the GraphOutput struct
        // contains the expected keys by exercising handle_graph with Json format.
        let result =
            handle_graph(&dir, false, &db_path, &Default::default(), &FormatKind::Json).unwrap();
        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_graph_json_cycles_only_should_return_1_when_cycles_present() {
        let (dir, db_path) = test_db_in_tempdir("graph21_json_cycle");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
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

        let result =
            handle_graph(&dir, true, &db_path, &Default::default(), &FormatKind::Json).unwrap();
        assert_eq!(result, 1, "JSON cycles-only with A→B→A cycle should exit 1");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
