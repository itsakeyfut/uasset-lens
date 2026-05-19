use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;

use crate::FormatKind;

#[derive(serde::Serialize)]
struct TypeStat {
    #[serde(rename = "type")]
    asset_type: String,
    count: usize,
    bytes: u64,
}

#[derive(serde::Serialize)]
struct FolderStat {
    folder: String,
    bytes: u64,
}

#[derive(serde::Serialize)]
struct LargestAsset {
    path: String,
    #[serde(rename = "type")]
    asset_type: String,
    bytes: u64,
}

#[derive(serde::Serialize)]
struct GraphStat {
    edges: usize,
    avg_out_degree: f64,
    unreferenced: usize,
    cycles: usize,
}

#[derive(serde::Serialize)]
struct StatsOutput {
    total_assets: usize,
    total_bytes: u64,
    by_type: Vec<TypeStat>,
    by_folder: Vec<FolderStat>,
    largest: Vec<LargestAsset>,
    graph: GraphStat,
}

pub fn handle_stats(
    project_dir: &Path,
    top: Option<usize>,
    db_path: &Path,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    let db = crate::open_db(db_path)?;
    let records = db
        .all_assets()
        .context("Failed to read assets from database")?;
    let config = crate::config::load_config(project_dir);
    let graph = crate::load_graph(&db, &config.scan.external_roots)?;

    let folder_limit = top.unwrap_or(5);
    let asset_limit = top.unwrap_or(10);

    let total_assets = records.len();
    let total_bytes: u64 = records.iter().map(|r| r.file_size).sum();

    let mut type_map: HashMap<String, (usize, u64)> = HashMap::new();
    for r in &records {
        let ty = r.asset_type.to_string();
        let (cnt, size) = type_map.entry(ty).or_default();
        *cnt += 1;
        *size += r.file_size;
    }
    let mut by_type: Vec<TypeStat> = type_map
        .into_iter()
        .map(|(asset_type, (count, bytes))| TypeStat {
            asset_type,
            count,
            bytes,
        })
        .collect();
    by_type.sort_unstable_by_key(|t| std::cmp::Reverse(t.bytes));

    let mut folder_map: HashMap<String, u64> = HashMap::new();
    for r in &records {
        let key = crate::path_depth_prefix(r.asset_path.as_str()).to_owned();
        *folder_map.entry(key).or_default() += r.file_size;
    }
    let mut by_folder: Vec<FolderStat> = folder_map
        .into_iter()
        .map(|(folder, bytes)| FolderStat { folder, bytes })
        .collect();
    by_folder.sort_unstable_by_key(|f| std::cmp::Reverse(f.bytes));
    by_folder.truncate(folder_limit);

    let mut sorted_by_size: Vec<&asset_db::AssetRecord> = records.iter().collect();
    sorted_by_size.sort_unstable_by_key(|r| std::cmp::Reverse(r.file_size));
    let largest: Vec<LargestAsset> = sorted_by_size
        .iter()
        .take(asset_limit)
        .map(|r| LargestAsset {
            path: r.asset_path.as_str().to_owned(), // clone required: AssetPath is not Copy
            asset_type: r.asset_type.to_string(),
            bytes: r.file_size,
        })
        .collect();

    let total_edges = graph.edge_count();
    let avg_out_degree = if total_assets > 0 {
        total_edges as f64 / total_assets as f64
    } else {
        0.0
    };
    let unreferenced = graph
        .nodes()
        .filter(|n| graph.in_degree(&n.path) == 0)
        .count();
    let cycle_count = graph.find_cycles().len();

    let graph_stat = GraphStat {
        edges: total_edges,
        avg_out_degree,
        unreferenced,
        cycles: cycle_count,
    };

    match format {
        FormatKind::Json => {
            let output = StatsOutput {
                total_assets,
                total_bytes,
                by_type,
                by_folder,
                largest,
                graph: graph_stat,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&output)
                    .context("Failed to serialize stats output to JSON")?
            );
        }
        FormatKind::GithubActions | FormatKind::Text => {
            let project_name = project_dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Project".to_string());

            println!(
                "Project Statistics \u{2014} {} ({} assets, {})",
                project_name,
                total_assets,
                crate::format_size(total_bytes)
            );

            // By Type: always show top 3, then "(N more types)" if any remain
            println!();
            println!("By Type:");
            let top3 = &by_type[..by_type.len().min(3)];
            if !top3.is_empty() {
                let max_name = top3.iter().map(|t| t.asset_type.len()).max().unwrap_or(1);
                let max_count = top3
                    .iter()
                    .map(|t| crate::digit_count(t.count))
                    .max()
                    .unwrap_or(1);
                let max_size_len = top3
                    .iter()
                    .map(|t| crate::format_size(t.bytes).len())
                    .max()
                    .unwrap_or(1);
                let max_bytes = top3.first().map(|t| t.bytes).unwrap_or(1).max(1);
                for t in top3 {
                    let size_str = crate::format_size(t.bytes);
                    let filled = (24.0 * t.bytes as f64 / max_bytes as f64).round() as usize;
                    let bar = "\u{2588}".repeat(filled) + &"\u{2591}".repeat(24 - filled);
                    let pct = if total_bytes > 0 {
                        100.0 * t.bytes as f64 / total_bytes as f64
                    } else {
                        0.0
                    };
                    println!(
                        "  {:<name$}  {:>cnt$}  {:>size$}  {}  {:.1}%",
                        t.asset_type,
                        t.count,
                        size_str,
                        bar,
                        pct,
                        name = max_name,
                        cnt = max_count,
                        size = max_size_len,
                    );
                }
            }
            let remaining = by_type.len().saturating_sub(3);
            if remaining > 0 {
                println!("  ({} more types)", remaining);
            }

            // By Folder
            println!();
            println!("By Folder (top {} by size):", folder_limit);
            if !by_folder.is_empty() {
                let max_folder = by_folder.iter().map(|f| f.folder.len()).max().unwrap_or(1);
                for f in &by_folder {
                    println!(
                        "  {:<folder$}  {}",
                        f.folder,
                        crate::format_size(f.bytes),
                        folder = max_folder,
                    );
                }
            }

            // Largest Assets
            println!();
            println!("Largest Assets (top {}):", asset_limit);
            for la in &largest {
                let name = la.path.split('/').next_back().unwrap_or(la.path.as_str());
                println!(
                    "  {}  {}  {}",
                    name,
                    crate::format_size(la.bytes),
                    la.asset_type
                );
            }

            // Graph
            println!();
            println!("Graph:");
            println!(
                "  Total edges      : {}",
                crate::format_number(graph_stat.edges)
            );
            println!("  Avg out-degree   : {:.2}", graph_stat.avg_out_degree);
            let pct_unref = if total_assets > 0 {
                100.0 * graph_stat.unreferenced as f64 / total_assets as f64
            } else {
                0.0
            };
            println!(
                "  Unreferenced     : {} ({:.1}%)",
                graph_stat.unreferenced, pct_unref,
            );
            println!("  Circular deps    : {}", graph_stat.cycles);
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
    fn handle_stats_should_return_err_when_db_does_not_exist() {
        let (dir, db_path) = test_db_in_tempdir("stats158_missing");
        let _ = std::fs::remove_dir_all(&dir);

        let result = handle_stats(Path::new("/proj"), None, &db_path, &FormatKind::Text);
        assert!(result.is_err(), "missing DB should return an error");
    }

    #[test]
    fn handle_stats_should_return_0_when_db_is_empty() {
        let (dir, db_path) = test_db_in_tempdir("stats158_empty");
        asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_stats(&dir, None, &db_path, &FormatKind::Text).unwrap();
        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_stats_should_return_0_when_assets_exist() {
        let (dir, db_path) = test_db_in_tempdir("stats158_found");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/Characters/BP_Hero",
                dir.join("BP_Hero.uasset"),
                AssetType::Blueprint,
                4096,
                vec![],
            )])
            .unwrap();
        }

        let result = handle_stats(&dir, None, &db_path, &FormatKind::Text).unwrap();
        assert_eq!(result, 0, "stats is informational — always exits 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_stats_json_should_contain_required_top_level_keys() {
        let (dir, db_path) = test_db_in_tempdir("stats158_json");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/T_Rock",
                dir.join("T_Rock.uasset"),
                AssetType::Texture2D,
                8192,
                vec![],
            )])
            .unwrap();
        }

        let db = asset_db::AssetDb::open_existing(&db_path).unwrap();
        let records = db.all_assets().unwrap();
        let graph = crate::load_graph(&db, &[]).unwrap();
        let output = StatsOutput {
            total_assets: records.len(),
            total_bytes: records.iter().map(|r| r.file_size).sum(),
            by_type: vec![TypeStat {
                asset_type: "Texture2D".to_owned(),
                count: 1,
                bytes: 8192,
            }],
            by_folder: vec![FolderStat {
                folder: "/Game/".to_owned(),
                bytes: 8192,
            }],
            largest: vec![LargestAsset {
                path: "/Game/T_Rock".to_owned(),
                asset_type: "Texture2D".to_owned(),
                bytes: 8192,
            }],
            graph: GraphStat {
                edges: graph.edge_count(),
                avg_out_degree: 0.0,
                unreferenced: 1,
                cycles: 0,
            },
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"total_assets\""), "must have total_assets");
        assert!(json.contains("\"total_bytes\""), "must have total_bytes");
        assert!(json.contains("\"by_type\""), "must have by_type");
        assert!(json.contains("\"by_folder\""), "must have by_folder");
        assert!(json.contains("\"largest\""), "must have largest");
        assert!(json.contains("\"graph\""), "must have graph");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_stats_json_should_compute_graph_stats_correctly() {
        let (dir, db_path) = test_db_in_tempdir("stats158_graph");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            // A → B (one edge); C is unreferenced and isolated; A→B is a cycle
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
                    2048,
                    vec![AssetPath::new("/Game/A").unwrap()],
                ),
                make_meta(
                    "/Game/C",
                    dir.join("C.uasset"),
                    AssetType::Texture2D,
                    512,
                    vec![],
                ),
            ])
            .unwrap();
        }

        // Use JSON format to trigger serialisation; check values via the handler return code.
        let result = handle_stats(&dir, None, &db_path, &FormatKind::Json).unwrap();
        assert_eq!(result, 0, "stats always exits 0");

        let db = asset_db::AssetDb::open_existing(&db_path).unwrap();
        let graph = crate::load_graph(&db, &[]).unwrap();
        assert_eq!(graph.edge_count(), 2, "A→B and B→A = 2 edges");
        let unreferenced = graph
            .nodes()
            .filter(|n| graph.in_degree(&n.path) == 0)
            .count();
        assert_eq!(unreferenced, 1, "only C has in_degree 0");
        assert_eq!(graph.find_cycles().len(), 1, "A↔B forms one cycle");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_stats_top_should_limit_folder_and_asset_output() {
        let (dir, db_path) = test_db_in_tempdir("stats158_top");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/A/BP_One",
                    dir.join("BP_One.uasset"),
                    AssetType::Blueprint,
                    8192,
                    vec![],
                ),
                make_meta(
                    "/Game/B/BP_Two",
                    dir.join("BP_Two.uasset"),
                    AssetType::Blueprint,
                    4096,
                    vec![],
                ),
                make_meta(
                    "/Game/C/BP_Three",
                    dir.join("BP_Three.uasset"),
                    AssetType::Blueprint,
                    2048,
                    vec![],
                ),
            ])
            .unwrap();
        }

        let result = handle_stats(&dir, Some(2), &db_path, &FormatKind::Text).unwrap();
        assert_eq!(
            result, 0,
            "stats is informational — always exits 0 even with --top"
        );

        let db = asset_db::AssetDb::open_existing(&db_path).unwrap();
        let records = db.all_assets().unwrap();

        let mut folder_map: HashMap<String, u64> = HashMap::new();
        for r in &records {
            *folder_map
                .entry(crate::path_depth_prefix(r.asset_path.as_str()).to_owned())
                .or_default() += r.file_size;
        }
        let mut by_folder: Vec<FolderStat> = folder_map
            .into_iter()
            .map(|(folder, bytes)| FolderStat { folder, bytes })
            .collect();
        by_folder.sort_unstable_by_key(|f| std::cmp::Reverse(f.bytes));
        by_folder.truncate(2);

        let mut sorted: Vec<&asset_db::AssetRecord> = records.iter().collect();
        sorted.sort_unstable_by_key(|r| std::cmp::Reverse(r.file_size));
        let largest: Vec<LargestAsset> = sorted
            .iter()
            .take(2)
            .map(|r| LargestAsset {
                path: r.asset_path.as_str().to_owned(),
                asset_type: r.asset_type.to_string(),
                bytes: r.file_size,
            })
            .collect();

        assert!(by_folder.len() <= 2, "folder list capped at top limit");
        assert!(largest.len() <= 2, "largest list capped at top limit");
        assert_eq!(largest[0].bytes, 8192, "largest asset is BP_One");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
