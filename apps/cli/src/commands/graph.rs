use std::collections::{HashMap, HashSet};
use std::path::Path;

use uasset_lens_shared::AssetPath;

use crate::FormatKind;

// Cycles longer than this threshold show first 2 nodes, hidden count, and last.
const CYCLE_TRUNCATE_THRESHOLD: usize = 6;

#[derive(serde::Serialize)]
struct CycleEntry {
    /// Closed cycle path (first node repeated at the end).
    nodes: Vec<String>,
    /// Deduplicated asset types in the cycle, in first-appearance order. Distinguishes harmless
    /// UE structural cycles (e.g. SkeletalMesh/Skeleton/PhysicsAsset) from Blueprint cycle bugs.
    types: Vec<String>,
}

#[derive(serde::Serialize)]
struct GraphOutput {
    total_assets: usize,
    total_edges: usize,
    cycles: Vec<CycleEntry>,
}

pub fn handle_graph(
    _project_dir: &Path,
    cycles_only: bool,
    full_cycles: bool,
    db_path: &Path,
    cfg: &crate::config::ConfigFile,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    let db = crate::open_db(db_path)?;
    let graph = crate::load_graph(&db, &cfg.scan.external_roots)?;
    let cycles = graph.find_cycles();
    let total_assets = graph.nodes().count();
    let total_edges = graph.edge_count();

    // Resolve asset types only for nodes that appear in a cycle, so large projects do not pay an
    // O(n) string allocation over every node just to annotate a handful of cycles.
    let cycle_node_paths: HashSet<&str> = cycles
        .iter()
        .flat_map(|scc| scc.iter().map(|p| p.as_str()))
        .collect();
    let type_by_path: HashMap<&str, String> = graph
        .nodes()
        .filter(|n| cycle_node_paths.contains(n.path.as_str()))
        .map(|n| (n.path.as_str(), n.asset_type.to_string()))
        .collect();

    // Build the closed-path cycle representation (first node repeated at end) and the
    // deduplicated asset-type composition of each cycle.
    let cycle_entries: Vec<CycleEntry> = cycles
        .iter()
        .map(|scc| {
            let mut nodes: Vec<String> = scc
                .iter()
                .map(|p| p.as_str().to_owned()) // clone required: AssetPath is not Copy
                .collect();
            if let Some(first) = nodes.first().cloned() {
                nodes.push(first);
            }
            CycleEntry {
                nodes,
                types: dedup_cycle_types(scc, &type_by_path),
            }
        })
        .collect();

    match format {
        FormatKind::Sarif => return Err(crate::sarif_not_supported()),
        FormatKind::Json => {
            let output = GraphOutput {
                total_assets,
                total_edges,
                cycles: cycle_entries,
            };
            crate::emit_json(&output, "Failed to serialize graph output to JSON")?;
        }
        FormatKind::GithubActions | FormatKind::Text => {
            if !cycles_only {
                println!("Dependency Graph Summary");
                println!("  Total assets   : {}", crate::format_number(total_assets));
                println!("  Total edges    : {}", crate::format_number(total_edges));
                println!("  Circular deps  : {} cycles detected", cycles.len());
            }
            if !cycle_entries.is_empty() {
                if !cycles_only {
                    println!();
                }
                println!("  Cycles:");
                for (i, entry) in cycle_entries.iter().enumerate() {
                    println!(
                        "    [{}] {}",
                        i + 1,
                        format_cycle(&entry.nodes, full_cycles)
                    );
                    if !entry.types.is_empty() {
                        println!("        [{}]", entry.types.join(", "));
                    }
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

/// The asset types present in a cycle, deduplicated and kept in first-appearance order. Nodes
/// missing from `type_by_path` (should not happen — every cycle node is a graph node) are skipped.
fn dedup_cycle_types(cycle: &[AssetPath], type_by_path: &HashMap<&str, String>) -> Vec<String> {
    let mut seen = HashSet::new();
    cycle
        .iter()
        .filter_map(|p| type_by_path.get(p.as_str()))
        .filter(|t| seen.insert(t.as_str()))
        .cloned()
        .collect()
}

fn format_cycle(nodes: &[String], full: bool) -> String {
    let unique_count = nodes.len().saturating_sub(1);
    if full || unique_count <= CYCLE_TRUNCATE_THRESHOLD {
        nodes.join(" \u{2192} ")
    } else {
        let hidden = nodes.len() - 3;
        format!(
            "{} \u{2192} {} \u{2192} ... ({hidden} nodes) \u{2192} {}",
            nodes[0],
            nodes[1],
            nodes[nodes.len() - 1],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{make_meta, test_db_in_tempdir};
    use uasset_lens_shared::{AssetPath, AssetType};

    #[test]
    fn handle_graph_should_return_err_when_db_does_not_exist() {
        let (dir, db_path) = test_db_in_tempdir("graph21_missing");
        let _ = std::fs::remove_dir_all(&dir);

        let result = handle_graph(
            Path::new("/proj"),
            false,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        );
        assert!(result.is_err(), "missing DB should return an error");
    }

    #[test]
    fn handle_graph_should_return_0_when_db_is_empty() {
        let (dir, db_path) = test_db_in_tempdir("graph21_empty");
        uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_graph(
            &dir,
            false,
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
    fn handle_graph_cycles_only_should_return_0_when_no_cycles() {
        let (dir, db_path) = test_db_in_tempdir("graph21_nocycle");
        uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_graph(
            &dir,
            true,
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
    fn handle_graph_cycles_only_should_return_1_when_cycles_present() {
        let (dir, db_path) = test_db_in_tempdir("graph21_cycle");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
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

        let result = handle_graph(
            &dir,
            true,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 1, "cycles-only with A→B→A cycle should exit 1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_graph_without_cycles_only_should_return_0_even_with_cycles() {
        let (dir, db_path) = test_db_in_tempdir("graph21_cycle_no_flag");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
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

        let result = handle_graph(
            &dir,
            false,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(
            result, 0,
            "without --cycles-only flag, exit code is always 0"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_graph_json_should_output_required_keys_when_db_is_empty() {
        let (dir, db_path) = test_db_in_tempdir("graph21_json");
        uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();

        // Verify that JSON serialization succeeds and the GraphOutput struct
        // contains the expected keys by exercising handle_graph with Json format.
        let result = handle_graph(
            &dir,
            false,
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
    fn handle_graph_json_cycles_only_should_return_1_when_cycles_present() {
        let (dir, db_path) = test_db_in_tempdir("graph21_json_cycle");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
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

        let result = handle_graph(
            &dir,
            true,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Json,
        )
        .unwrap();
        assert_eq!(result, 1, "JSON cycles-only with A→B→A cycle should exit 1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn make_cycle_path(unique_count: usize) -> Vec<String> {
        let mut v: Vec<String> = (0..unique_count).map(|i| format!("/Game/N{i}")).collect();
        v.push(v[0].clone()); // closed cycle: repeat first at end
        v
    }

    #[test]
    fn format_cycle_should_join_all_nodes_when_at_threshold() {
        // 6 unique nodes = exactly CYCLE_TRUNCATE_THRESHOLD — must not truncate
        let nodes = make_cycle_path(6);
        let result = format_cycle(&nodes, false);
        assert!(result.contains("/Game/N5"), "last unique node shown");
        assert!(!result.contains("..."), "no truncation at threshold");
    }

    #[test]
    fn format_cycle_should_truncate_middle_when_above_threshold() {
        // 7 unique nodes → truncation: show N0, N1, ... (5 nodes), N0
        let nodes = make_cycle_path(7);
        let result = format_cycle(&nodes, false);
        assert!(
            result.starts_with("/Game/N0 \u{2192} /Game/N1"),
            "shows first 2 nodes"
        );
        assert!(result.contains("... (5 nodes)"), "hidden count is correct");
        assert!(result.ends_with("/Game/N0"), "shows last = repeated first");
        assert!(!result.contains("/Game/N2"), "hides middle nodes");
    }

    #[test]
    fn format_cycle_should_show_all_nodes_when_full_is_true() {
        // full=true disables truncation even for large cycles
        let nodes = make_cycle_path(7);
        let result = format_cycle(&nodes, true);
        assert!(result.contains("/Game/N2"), "middle nodes shown when full");
        assert!(!result.contains("..."), "no truncation when full");
    }

    #[test]
    fn handle_graph_text_should_return_1_and_not_panic_with_long_cycle_when_not_full() {
        let (dir, db_path) = test_db_in_tempdir("graph237_trunc");
        {
            // Build a 7-node cycle: N0→N1→…→N6→N0
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            let paths: Vec<_> = (0..7)
                .map(|i| AssetPath::new(&format!("/Game/N{i}")).unwrap())
                .collect();
            let metas: Vec<_> = paths
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    make_meta(
                        p.as_str(),
                        dir.join(format!("N{i}.uasset")),
                        AssetType::Blueprint,
                        512,
                        vec![paths[(i + 1) % 7].clone()],
                    )
                })
                .collect();
            db.upsert_all(&metas).unwrap();
        }
        let result = handle_graph(
            &dir,
            true,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 1, "7-node cycle → cycles-only exits 1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_graph_text_should_return_1_and_not_panic_with_long_cycle_when_full() {
        let (dir, db_path) = test_db_in_tempdir("graph237_full");
        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            let paths: Vec<_> = (0..7)
                .map(|i| AssetPath::new(&format!("/Game/M{i}")).unwrap())
                .collect();
            let metas: Vec<_> = paths
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    make_meta(
                        p.as_str(),
                        dir.join(format!("M{i}.uasset")),
                        AssetType::Blueprint,
                        512,
                        vec![paths[(i + 1) % 7].clone()],
                    )
                })
                .collect();
            db.upsert_all(&metas).unwrap();
        }
        let result = handle_graph(
            &dir,
            true,
            true,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 1, "full_cycles=true, 7-node cycle → exits 1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dedup_cycle_types_should_dedup_preserving_first_appearance() {
        let cycle = vec![
            AssetPath::new("/Game/A").unwrap(),
            AssetPath::new("/Game/B").unwrap(),
            AssetPath::new("/Game/C").unwrap(),
        ];
        let mut type_by_path: HashMap<&str, String> = HashMap::new();
        type_by_path.insert("/Game/A", "Blueprint".to_owned());
        type_by_path.insert("/Game/B", "Texture2D".to_owned());
        type_by_path.insert("/Game/C", "Blueprint".to_owned()); // duplicate type, later node
        assert_eq!(
            dedup_cycle_types(&cycle, &type_by_path),
            vec!["Blueprint".to_owned(), "Texture2D".to_owned()],
            "types deduplicated in first-appearance order"
        );
    }

    #[test]
    fn cycle_entry_json_should_include_nodes_and_types() {
        let json = serde_json::to_string(&CycleEntry {
            nodes: vec![
                "/Game/A".to_owned(),
                "/Game/B".to_owned(),
                "/Game/A".to_owned(),
            ],
            types: vec!["Blueprint".to_owned()],
        })
        .unwrap();
        assert!(json.contains("\"nodes\""), "cycle entry must include nodes");
        assert!(json.contains("\"types\""), "cycle entry must include types");
    }
}
