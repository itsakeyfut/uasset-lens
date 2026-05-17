use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use anyhow::Context;
use shared::{AssetPath, AssetType};

use crate::FormatKind;

#[derive(serde::Serialize)]
struct DepNode {
    path: String,
    #[serde(rename = "type")]
    asset_type: String,
    size_bytes: u64,
    deps: Vec<DepNode>,
}

pub fn handle_deps(
    asset_path: &Path,
    db_override: Option<&Path>,
    depth: Option<u32>,
    size_only: bool,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    let (target, db_path) = resolve_target_and_db(asset_path, db_override)?;
    let db = crate::open_db(&db_path)?;
    let graph = crate::load_graph(&db)?;

    if !graph.contains(&target) {
        anyhow::bail!("asset not found in scan data: {}", target.as_str());
    }

    let records = db
        .all_assets()
        .context("Failed to read assets from database")?;
    let asset_map: HashMap<AssetPath, (AssetType, u64)> = records
        .into_iter()
        .map(|r| (r.asset_path, (r.asset_type, r.file_size)))
        .collect();

    let max_depth = depth.unwrap_or(u32::MAX);
    let (direct_count, transitive_count, total_size) = compute_stats(&graph, &asset_map, &target);

    match format {
        FormatKind::Json => {
            if size_only {
                let stats = serde_json::json!({
                    "root": target.as_str(),
                    "direct": direct_count,
                    "transitive": transitive_count,
                    "total_size_bytes": total_size,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&stats)
                        .context("Failed to serialize deps stats to JSON")?
                );
            } else {
                let mut in_path = HashSet::new();
                let root_node =
                    build_json_node(&graph, &asset_map, &target, &mut in_path, 0, max_depth);
                println!(
                    "{}",
                    serde_json::to_string_pretty(&root_node)
                        .context("Failed to serialize deps tree to JSON")?
                );
            }
        }
        FormatKind::Text => {
            if !size_only {
                let (root_type, root_size) = asset_map
                    .get(&target)
                    .map(|(t, s)| (t.to_string(), *s))
                    .unwrap_or_else(|| ("Unknown".to_string(), 0));
                println!(
                    "{}  ({}, {})",
                    target.as_str(),
                    root_type,
                    crate::format_size(root_size)
                );

                let mut in_path: HashSet<AssetPath> = HashSet::new();
                in_path.insert(target.clone()); // clone required: target still needed as ref for dependency lookup
                let direct_deps = graph.dependencies_of(&target);
                for (i, (dep_path, dep_type)) in direct_deps.iter().enumerate() {
                    let is_last = i == direct_deps.len() - 1;
                    print_tree_node(
                        &graph,
                        &asset_map,
                        dep_path,
                        dep_type,
                        &mut in_path,
                        1,
                        max_depth,
                        "",
                        is_last,
                    );
                }
                println!();
            }
            println!(
                "Direct: {}   Transitive: {}   Total size: {}",
                direct_count,
                transitive_count,
                crate::format_size(total_size)
            );
        }
    }

    Ok(0)
}

fn compute_stats(
    graph: &dependency_graph::DependencyGraph,
    asset_map: &HashMap<AssetPath, (AssetType, u64)>,
    target: &AssetPath,
) -> (usize, usize, u64) {
    let root_size = asset_map.get(target).map(|(_, s)| *s).unwrap_or(0);
    let mut visited: HashSet<AssetPath> = HashSet::new();
    visited.insert(target.clone()); // clone required: target is borrowed
    let mut total_size = root_size;
    let mut direct_count = 0;
    let mut transitive_count = 0;
    let mut queue: VecDeque<AssetPath> = VecDeque::new();

    for (dep_path, _) in graph.dependencies_of(target) {
        if visited.insert(dep_path.clone()) {
            // clone required: dep_path also moved into queue
            total_size += asset_map.get(&dep_path).map(|(_, s)| *s).unwrap_or(0);
            direct_count += 1;
            queue.push_back(dep_path);
        }
    }

    while let Some(path) = queue.pop_front() {
        for (dep_path, _) in graph.dependencies_of(&path) {
            if visited.insert(dep_path.clone()) {
                // clone required: dep_path also moved into queue
                total_size += asset_map.get(&dep_path).map(|(_, s)| *s).unwrap_or(0);
                transitive_count += 1;
                queue.push_back(dep_path);
            }
        }
    }

    (direct_count, transitive_count, total_size)
}

// Each parameter maps to a distinct tree-rendering concern; extracting a struct adds indirection without simplifying call sites.
#[allow(clippy::too_many_arguments)]
fn print_tree_node(
    graph: &dependency_graph::DependencyGraph,
    asset_map: &HashMap<AssetPath, (AssetType, u64)>,
    path: &AssetPath,
    asset_type: &AssetType,
    in_path: &mut HashSet<AssetPath>,
    depth: u32,
    max_depth: u32,
    prefix: &str,
    is_last: bool,
) {
    let connector = if is_last { "└── " } else { "├── " };
    let size = asset_map.get(path).map(|(_, s)| *s).unwrap_or(0);

    if in_path.contains(path) {
        println!(
            "{}{}{}  ({}, {}) [cycle]",
            prefix,
            connector,
            path.as_str(),
            asset_type,
            crate::format_size(size)
        );
        return;
    }

    println!(
        "{}{}{}  ({}, {})",
        prefix,
        connector,
        path.as_str(),
        asset_type,
        crate::format_size(size)
    );

    if depth >= max_depth {
        let child_count = graph.dependencies_of(path).len();
        if child_count > 0 {
            let child_prefix = if is_last {
                format!("{}    ", prefix)
            } else {
                format!("{}│   ", prefix)
            };
            println!(
                "{}└── ... ({} more at depth {})",
                child_prefix,
                child_count,
                depth + 1
            );
        }
        return;
    }

    let child_prefix = if is_last {
        format!("{}    ", prefix)
    } else {
        format!("{}│   ", prefix)
    };
    let deps = graph.dependencies_of(path);
    in_path.insert(path.clone()); // clone required: AssetPath is not Copy
    for (i, (dep_path, dep_type)) in deps.iter().enumerate() {
        let is_last_child = i == deps.len() - 1;
        print_tree_node(
            graph,
            asset_map,
            dep_path,
            dep_type,
            in_path,
            depth + 1,
            max_depth,
            &child_prefix,
            is_last_child,
        );
    }
    in_path.remove(path);
}

fn build_json_node(
    graph: &dependency_graph::DependencyGraph,
    asset_map: &HashMap<AssetPath, (AssetType, u64)>,
    path: &AssetPath,
    in_path: &mut HashSet<AssetPath>,
    depth: u32,
    max_depth: u32,
) -> DepNode {
    let (type_str, size) = asset_map
        .get(path)
        .map(|(t, s)| (t.to_string(), *s))
        .unwrap_or_else(|| ("Unknown".to_string(), 0));

    if in_path.contains(path) || depth >= max_depth {
        return DepNode {
            path: path.as_str().to_owned(),
            asset_type: type_str,
            size_bytes: size,
            deps: vec![],
        };
    }

    let raw_deps = graph.dependencies_of(path);
    in_path.insert(path.clone()); // clone required: AssetPath is not Copy
    let mut deps = Vec::with_capacity(raw_deps.len());
    for (dep_path, _) in &raw_deps {
        deps.push(build_json_node(
            graph,
            asset_map,
            dep_path,
            in_path,
            depth + 1,
            max_depth,
        ));
    }
    in_path.remove(path);

    DepNode {
        path: path.as_str().to_owned(),
        asset_type: type_str,
        size_bytes: size,
        deps,
    }
}

fn resolve_target_and_db(
    asset_path: &Path,
    db_override: Option<&Path>,
) -> anyhow::Result<(AssetPath, PathBuf)> {
    if asset_path.starts_with("/Game") {
        let s = asset_path.to_string_lossy();
        let target = AssetPath::new(&s).map_err(|e| anyhow::anyhow!("invalid game path: {e}"))?;
        let db_path = match db_override {
            Some(p) => p.to_path_buf(),
            None => {
                let cwd = std::env::current_dir().context("Failed to get current directory")?;
                let project_dir = find_project_dir(&cwd)?;
                project_dir.join(".uasset-lens").join("uasset-lens.db")
            }
        };
        return Ok((target, db_path));
    }

    let start = asset_path.parent().unwrap_or(asset_path);
    let project_dir = find_project_dir(start)?;
    let content_root = crate::resolve_content_root(&project_dir);
    let target = AssetPath::from_fs_path(&content_root, asset_path)
        .map_err(|e| anyhow::anyhow!("cannot convert path to game path: {e}"))?;
    let db_path = db_override
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| project_dir.join(".uasset-lens").join("uasset-lens.db"));

    Ok((target, db_path))
}

fn find_project_dir(start: &Path) -> anyhow::Result<PathBuf> {
    let mut dir = start;
    loop {
        if dir.join(".uasset-lens").is_dir() {
            return Ok(dir.to_path_buf());
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => {
                anyhow::bail!("no scan data found.\nRun 'uasset-lens scan <project_dir>' first.")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::make_meta;
    use shared::{AssetPath, AssetType};

    #[test]
    fn handle_deps_should_return_err_when_db_does_not_exist() {
        let db_path = std::env::temp_dir().join(format!(
            "uasset_lens_deps_missing_{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);

        let result = handle_deps(
            Path::new("/Game/Target"),
            Some(&db_path),
            None,
            false,
            &FormatKind::Text,
        );
        assert!(result.is_err(), "missing DB should return an error");
    }

    #[test]
    fn handle_deps_should_return_err_when_target_not_in_graph() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_deps_notfound_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_deps(
            Path::new("/Game/NotInGraph"),
            Some(&db_path),
            None,
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
    fn handle_deps_should_return_0_when_no_dependencies() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_deps_no_deps_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/Leaf",
                dir.join("Leaf.uasset"),
                AssetType::Blueprint,
                1024,
                vec![],
            )])
            .unwrap();
        }

        let result = handle_deps(
            Path::new("/Game/Leaf"),
            Some(&db_path),
            None,
            false,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "leaf asset with no deps should exit 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_deps_should_return_0_when_dependencies_exist() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_deps_with_deps_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/Root",
                    dir.join("Root.uasset"),
                    AssetType::Blueprint,
                    2048,
                    vec![AssetPath::new("/Game/Dep").unwrap()],
                ),
                make_meta(
                    "/Game/Dep",
                    dir.join("Dep.uasset"),
                    AssetType::StaticMesh,
                    1024,
                    vec![],
                ),
            ])
            .unwrap();
        }

        let result = handle_deps(
            Path::new("/Game/Root"),
            Some(&db_path),
            None,
            false,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "asset with deps should exit 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_deps_json_should_serialize_nested_structure() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_deps_json_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/Root",
                    dir.join("Root.uasset"),
                    AssetType::Blueprint,
                    2048,
                    vec![AssetPath::new("/Game/Dep").unwrap()],
                ),
                make_meta(
                    "/Game/Dep",
                    dir.join("Dep.uasset"),
                    AssetType::Texture2D,
                    512,
                    vec![],
                ),
            ])
            .unwrap();
        }

        let result = handle_deps(
            Path::new("/Game/Root"),
            Some(&db_path),
            None,
            false,
            &FormatKind::Json,
        )
        .unwrap();
        assert_eq!(result, 0, "JSON output should exit 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_deps_should_handle_cycles_without_infinite_loop() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_deps_cycle_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            // A → B → A (cycle)
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

        let result = handle_deps(
            Path::new("/Game/A"),
            Some(&db_path),
            None,
            false,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "cycle should be handled gracefully");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_deps_should_respect_depth_limit() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_deps_depth_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            // A → B → C (depth 2)
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
                    vec![AssetPath::new("/Game/C").unwrap()],
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

        let result = handle_deps(
            Path::new("/Game/A"),
            Some(&db_path),
            Some(1),
            false,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "depth-limited output should exit 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_json_node_should_produce_correct_path_type_size_and_nested_deps() {
        let graph = dependency_graph::DependencyGraph::build(
            vec![
                dependency_graph::AssetNode {
                    path: shared::AssetPath::new("/Game/Root").unwrap(),
                    asset_type: shared::AssetType::Blueprint,
                },
                dependency_graph::AssetNode {
                    path: shared::AssetPath::new("/Game/Dep").unwrap(),
                    asset_type: shared::AssetType::Texture2D,
                },
            ],
            vec![(
                shared::AssetPath::new("/Game/Root").unwrap(),
                shared::AssetPath::new("/Game/Dep").unwrap(),
            )],
        );
        let mut asset_map: HashMap<AssetPath, (AssetType, u64)> = HashMap::new();
        asset_map.insert(
            shared::AssetPath::new("/Game/Root").unwrap(),
            (shared::AssetType::Blueprint, 2048),
        );
        asset_map.insert(
            shared::AssetPath::new("/Game/Dep").unwrap(),
            (shared::AssetType::Texture2D, 512),
        );
        let mut in_path = HashSet::new();
        let node = build_json_node(
            &graph,
            &asset_map,
            &shared::AssetPath::new("/Game/Root").unwrap(),
            &mut in_path,
            0,
            u32::MAX,
        );
        assert_eq!(node.path, "/Game/Root");
        assert_eq!(node.asset_type, "Blueprint");
        assert_eq!(node.size_bytes, 2048);
        assert_eq!(node.deps.len(), 1, "root should have one direct dep");
        assert_eq!(node.deps[0].path, "/Game/Dep");
        assert_eq!(node.deps[0].asset_type, "Texture2D");
        assert_eq!(node.deps[0].size_bytes, 512);
        assert!(
            node.deps[0].deps.is_empty(),
            "leaf dep should have no children"
        );
    }

    #[test]
    fn handle_deps_size_only_should_return_0() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_deps_size_only_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/Root",
                    dir.join("Root.uasset"),
                    AssetType::Blueprint,
                    4096,
                    vec![AssetPath::new("/Game/Dep").unwrap()],
                ),
                make_meta(
                    "/Game/Dep",
                    dir.join("Dep.uasset"),
                    AssetType::Texture2D,
                    2048,
                    vec![],
                ),
            ])
            .unwrap();
        }

        let result = handle_deps(
            Path::new("/Game/Root"),
            Some(&db_path),
            None,
            true,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "size-only mode should exit 0");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
