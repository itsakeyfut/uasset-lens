use std::collections::HashSet;
use std::path::Path;

use anyhow::Context;
use uasset_lens_shared::AssetPath;

use crate::FormatKind;

#[derive(serde::Serialize)]
struct ImpactOutput {
    target: String,
    direct: Vec<String>,
    transitive: Vec<String>,
    total: usize,
}

#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "lowercase")]
enum NodeKind {
    Direct,
    Transitive,
    Cycle,
}

#[derive(serde::Serialize)]
struct TreeNode {
    path: String,
    kind: NodeKind,
    children: Vec<TreeNode>,
}

#[derive(serde::Serialize)]
struct ImpactTreeOutput {
    target: String,
    children: Vec<TreeNode>,
}

pub fn handle_impact(
    project_dir: &Path,
    asset_path: &Path,
    db_path: &Path,
    cfg: &crate::config::ConfigFile,
    tree: bool,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    let target = crate::resolve_asset_path(project_dir, asset_path)?;
    let db = crate::open_db(db_path)?;
    let graph = crate::load_graph(&db, &cfg.scan.external_roots)?;

    if !graph.contains(&target) {
        anyhow::bail!("asset not found in scan data: {}", target.as_str());
    }

    let result = graph.find_impact(&target);
    let total = result.direct.len() + result.transitive.len();

    if tree {
        let mut path_stack = HashSet::new();
        path_stack.insert(target.clone()); // clone required: path_stack owns the key
        let children: Vec<TreeNode> = graph
            .reverse_deps_of(&target)
            .into_iter()
            .map(|p| build_tree(&graph, p, 1, &mut path_stack))
            .collect();

        match format {
            FormatKind::Sarif => return Err(crate::sarif_not_supported()),
            FormatKind::Json => {
                let out = ImpactTreeOutput {
                    target: target.as_str().to_owned(),
                    children,
                };
                println!(
                    "{}",
                    serde_json::to_string_pretty(&out)
                        .context("Failed to serialize impact tree output to JSON")?
                );
            }
            FormatKind::GithubActions | FormatKind::Text => {
                println!("{}  [target]", target.as_str());
                let n = children.len();
                for (i, child) in children.iter().enumerate() {
                    print_tree_node(child, "", i == n - 1, None);
                }
                println!();
                println!(
                    "Impact: {} direct, {} transitive \u{2014} {} assets total",
                    result.direct.len(),
                    result.transitive.len(),
                    total
                );
            }
        }
        return if total > 0 { Ok(1) } else { Ok(0) };
    }

    match format {
        FormatKind::Sarif => return Err(crate::sarif_not_supported()),
        FormatKind::Json => {
            let output = ImpactOutput {
                target: target.as_str().to_owned(),
                direct: result
                    .direct
                    .iter()
                    .map(|p| p.as_str().to_owned()) // clone required: AssetPath is not Copy
                    .collect(),
                transitive: result
                    .transitive
                    .iter()
                    .map(|p| p.as_str().to_owned()) // clone required: AssetPath is not Copy
                    .collect(),
                total,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&output)
                    .context("Failed to serialize impact output to JSON")?
            );
        }
        FormatKind::GithubActions | FormatKind::Text => {
            println!("  Impact Analysis: {}", target.as_str());
            println!();
            println!("  Direct referencing ({}):", result.direct.len());
            for path in &result.direct {
                println!("    {}", path.as_str());
            }
            println!();
            println!("  Transitive referencing ({}):", result.transitive.len());
            let remaining = result.transitive.len().saturating_sub(10);
            for path in result.transitive.iter().take(10) {
                println!("    {}", path.as_str());
            }
            if remaining > 0 {
                println!("    ... ({} more)", remaining);
            }
            println!();
            println!("  Total impact: {} assets", total);
        }
    }

    if total > 0 { Ok(1) } else { Ok(0) }
}

fn build_tree(
    graph: &uasset_lens_dependency_graph::DependencyGraph,
    path: AssetPath,
    depth: usize,
    path_stack: &mut HashSet<AssetPath>,
) -> TreeNode {
    if path_stack.contains(&path) {
        return TreeNode {
            path: path.as_str().to_owned(),
            kind: NodeKind::Cycle,
            children: vec![],
        };
    }
    let kind = if depth == 1 {
        NodeKind::Direct
    } else {
        NodeKind::Transitive
    };
    path_stack.insert(path.clone()); // clone required: path_stack takes ownership of key
    let children = graph
        .reverse_deps_of(&path)
        .into_iter()
        .map(|p| build_tree(graph, p, depth + 1, path_stack))
        .collect();
    path_stack.remove(&path);
    TreeNode {
        path: path.as_str().to_owned(),
        kind,
        children,
    }
}

fn print_tree_node(node: &TreeNode, prefix: &str, is_last: bool, parent_short: Option<&str>) {
    let connector = if is_last {
        "\u{2514}\u{2500}\u{2500} "
    } else {
        "\u{251C}\u{2500}\u{2500} "
    };
    let annotation = match node.kind {
        NodeKind::Direct => "(direct)".to_owned(),
        NodeKind::Cycle => "[cycle]".to_owned(),
        NodeKind::Transitive => format!("(via {})", parent_short.unwrap_or("?")),
    };
    println!("{}{}{}  {}", prefix, connector, node.path, annotation);
    let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "\u{2502}   " });
    let short = node.path.split('/').next_back().unwrap_or(&node.path);
    let n = node.children.len();
    for (i, child) in node.children.iter().enumerate() {
        print_tree_node(child, &child_prefix, i == n - 1, Some(short));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{make_meta, test_db_in_tempdir};
    use uasset_lens_shared::{AssetPath, AssetType};

    #[test]
    fn handle_impact_should_return_err_when_db_does_not_exist() {
        let (dir, db_path) = test_db_in_tempdir("impact23_missing");
        let _ = std::fs::remove_dir_all(&dir);

        let result = handle_impact(
            &dir,
            Path::new("/Game/Target"),
            &db_path,
            &Default::default(),
            false,
            &FormatKind::Text,
        );
        assert!(result.is_err(), "missing DB should return an error");
    }

    #[test]
    fn handle_impact_should_return_err_when_target_not_in_graph() {
        let (dir, db_path) = test_db_in_tempdir("impact23_notfound");
        uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_impact(
            &dir,
            Path::new("/Game/NotInGraph"),
            &db_path,
            &Default::default(),
            false,
            &FormatKind::Text,
        );
        assert!(
            result.is_err(),
            "target not in graph should return an error"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_impact_should_return_0_when_no_referencing_assets() {
        let (dir, db_path) = test_db_in_tempdir("impact23_no_refs");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/Target",
                dir.join("Target.uasset"),
                AssetType::Blueprint,
                1024,
                vec![],
            )])
            .unwrap();
        }

        let result = handle_impact(
            &dir,
            Path::new("/Game/Target"),
            &db_path,
            &Default::default(),
            false,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "no referencing assets means no impact");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_impact_should_return_1_when_direct_references_exist() {
        let (dir, db_path) = test_db_in_tempdir("impact23_direct");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            // A→Target: A directly references Target
            db.upsert_all(&[
                make_meta(
                    "/Game/Target",
                    dir.join("Target.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
                make_meta(
                    "/Game/A",
                    dir.join("A.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![AssetPath::new("/Game/Target").unwrap()],
                ),
            ])
            .unwrap();
        }

        let result = handle_impact(
            &dir,
            Path::new("/Game/Target"),
            &db_path,
            &Default::default(),
            false,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 1, "one direct reference means impact exists");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_impact_should_correctly_separate_direct_and_transitive() {
        let (dir, db_path) = test_db_in_tempdir("impact23_transitive");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            // C→B→Target: B is direct, C is transitive
            db.upsert_all(&[
                make_meta(
                    "/Game/Target",
                    dir.join("Target.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
                make_meta(
                    "/Game/B",
                    dir.join("B.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![AssetPath::new("/Game/Target").unwrap()],
                ),
                make_meta(
                    "/Game/C",
                    dir.join("C.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![AssetPath::new("/Game/B").unwrap()],
                ),
            ])
            .unwrap();
        }

        let result = handle_impact(
            &dir,
            Path::new("/Game/Target"),
            &db_path,
            &Default::default(),
            false,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 1, "both direct and transitive refs make total > 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_impact_json_should_return_0_when_no_impact() {
        let (dir, db_path) = test_db_in_tempdir("impact23_json_empty");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/Target",
                dir.join("Target.uasset"),
                AssetType::Blueprint,
                1024,
                vec![],
            )])
            .unwrap();
        }

        let result = handle_impact(
            &dir,
            Path::new("/Game/Target"),
            &db_path,
            &Default::default(),
            false,
            &FormatKind::Json,
        )
        .unwrap();
        assert_eq!(result, 0, "JSON format exits 0 when no impact");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_impact_json_should_return_1_when_impact_exists() {
        let (dir, db_path) = test_db_in_tempdir("impact23_json_found");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/Target",
                    dir.join("Target.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
                make_meta(
                    "/Game/Referencing",
                    dir.join("Referencing.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![AssetPath::new("/Game/Target").unwrap()],
                ),
            ])
            .unwrap();
        }

        let result = handle_impact(
            &dir,
            Path::new("/Game/Target"),
            &db_path,
            &Default::default(),
            false,
            &FormatKind::Json,
        )
        .unwrap();
        assert_eq!(result, 1, "JSON format exits 1 when impact exists");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_project_dir_should_find_uasset_lens_dir_in_ancestor() {
        let (dir, _) = test_db_in_tempdir("impact23_projdir");
        let sub = dir.join("Content").join("Characters");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::create_dir_all(dir.join(".uasset-lens")).unwrap();

        let result = crate::find_project_dir(&sub).unwrap();
        assert_eq!(result, dir);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_project_dir_should_return_err_when_no_uasset_lens_dir_found() {
        // Nonexistent path under temp_dir — the system temp directory and its ancestors
        // are guaranteed to have no .uasset-lens (that dir only lives under the project root)
        let nonexistent = std::env::temp_dir().join(format!(
            "uasset_lens_impact23_projdir_miss_{}",
            std::process::id()
        ));
        assert!(
            crate::find_project_dir(&nonexistent).is_err(),
            "no .uasset-lens in path should return error"
        );
    }

    #[test]
    fn handle_impact_tree_should_return_0_when_no_impact() {
        let (dir, db_path) = test_db_in_tempdir("impact163_tree_empty");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/Target",
                dir.join("Target.uasset"),
                AssetType::Blueprint,
                1024,
                vec![],
            )])
            .unwrap();
        }

        let result = handle_impact(
            &dir,
            Path::new("/Game/Target"),
            &db_path,
            &Default::default(),
            true,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "tree mode: no referencing assets → exit 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_impact_tree_should_return_1_when_direct_references_exist() {
        let (dir, db_path) = test_db_in_tempdir("impact163_tree_direct");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/Target",
                    dir.join("Target.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
                make_meta(
                    "/Game/A",
                    dir.join("A.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![AssetPath::new("/Game/Target").unwrap()],
                ),
            ])
            .unwrap();
        }

        let result = handle_impact(
            &dir,
            Path::new("/Game/Target"),
            &db_path,
            &Default::default(),
            true,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 1, "tree mode: direct reference → exit 1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_impact_tree_json_should_return_1_when_reference_exists() {
        let (dir, db_path) = test_db_in_tempdir("impact163_tree_json");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/Target",
                    dir.join("Target.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
                make_meta(
                    "/Game/A",
                    dir.join("A.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![AssetPath::new("/Game/Target").unwrap()],
                ),
            ])
            .unwrap();
        }

        let result = handle_impact(
            &dir,
            Path::new("/Game/Target"),
            &db_path,
            &Default::default(),
            true,
            &FormatKind::Json,
        )
        .unwrap();
        assert_eq!(result, 1, "tree JSON mode: reference exists → exit 1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_impact_tree_should_not_infinite_loop_on_cycle() {
        let (dir, db_path) = test_db_in_tempdir("impact163_tree_cycle");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            // A→Target→A: mutual cycle
            db.upsert_all(&[
                make_meta(
                    "/Game/Target",
                    dir.join("Target.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![AssetPath::new("/Game/A").unwrap()],
                ),
                make_meta(
                    "/Game/A",
                    dir.join("A.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![AssetPath::new("/Game/Target").unwrap()],
                ),
            ])
            .unwrap();
        }

        // path_stack prevents infinite DFS when A→Target→A forms a mutual cycle
        let result = handle_impact(
            &dir,
            Path::new("/Game/Target"),
            &db_path,
            &Default::default(),
            true,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 1, "tree mode with cycle: impact exists → exit 1");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
