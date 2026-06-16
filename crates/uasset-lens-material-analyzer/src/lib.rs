use std::collections::HashSet;

use uasset_lens_dependency_graph::DependencyGraph;
use uasset_lens_scanner::AssetMetadata;
use uasset_lens_shared::{AssetPath, AssetType};

#[derive(Debug, Clone, PartialEq)]
pub struct MaterialMetrics {
    pub texture_sample_count: u32,
    pub instance_chain_depth: u32,
}

pub fn analyze(metadata: &AssetMetadata, graph: &DependencyGraph) -> Option<MaterialMetrics> {
    match &metadata.asset_type {
        AssetType::Material => Some(MaterialMetrics {
            texture_sample_count: metadata.material_texture_samples.unwrap_or(0),
            instance_chain_depth: 0,
        }),
        AssetType::MaterialInstance => Some(MaterialMetrics {
            texture_sample_count: 0,
            instance_chain_depth: walk_instance_chain(&metadata.asset_path, graph),
        }),
        _ => None,
    }
}

// Walks dependency edges from a MaterialInstance node, following MaterialInstance
// and Material nodes until the chain terminates. Counts hops (depth).
// A visited set prevents infinite looping on cyclic graphs.
fn walk_instance_chain(path: &AssetPath, graph: &DependencyGraph) -> u32 {
    let mut current = path.clone(); // clone required: walk state owns the current path
    let mut depth = 0u32;
    let mut visited: HashSet<AssetPath> = HashSet::new();
    visited.insert(current.clone()); // clone required: current is also used below

    loop {
        let deps = graph.dependencies_of(&current);
        let next = deps
            .into_iter()
            .find(|(_, t)| matches!(t, AssetType::MaterialInstance | AssetType::Material));
        match next {
            Some((next_path, AssetType::MaterialInstance)) if !visited.contains(&next_path) => {
                depth += 1;
                visited.insert(next_path.clone()); // clone required: next_path is moved below
                current = next_path;
            }
            Some((_, AssetType::Material)) => {
                depth += 1;
                break;
            }
            _ => break,
        }
    }
    depth
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use uasset_lens_dependency_graph::AssetNode;
    use uasset_lens_shared::AssetPath;

    const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/valid");

    fn ap(path: &str) -> AssetPath {
        AssetPath::new(path).unwrap()
    }

    fn node(path: &str, asset_type: AssetType) -> AssetNode {
        AssetNode {
            path: ap(path),
            asset_type,
        }
    }

    fn make_metadata(
        asset_path: &str,
        asset_type: AssetType,
        texture_samples: Option<u32>,
    ) -> AssetMetadata {
        AssetMetadata {
            asset_path: ap(asset_path),
            file_path: PathBuf::from("dummy.uasset"),
            asset_type,
            file_size: 0,
            last_modified: 0,
            dependencies: vec![],
            soft_dependencies: vec![],
            blueprint_metrics: None,
            material_texture_samples: texture_samples,
        }
    }

    #[test]
    fn analyze_should_return_none_for_non_material_assets() {
        let graph = DependencyGraph::build(vec![], vec![], &[] as &[&str]);
        assert!(
            analyze(
                &make_metadata("/Game/SM", AssetType::StaticMesh, None),
                &graph
            )
            .is_none()
        );
        assert!(
            analyze(
                &make_metadata("/Game/BP", AssetType::Blueprint, None),
                &graph
            )
            .is_none()
        );
    }

    #[test]
    fn analyze_should_propagate_texture_sample_count_from_metadata_for_material() {
        let graph = DependencyGraph::build(vec![], vec![], &[] as &[&str]);
        let meta = make_metadata("/Game/M_Test", AssetType::Material, Some(3));
        let result = analyze(&meta, &graph).expect("Material should return Some");
        assert_eq!(result.texture_sample_count, 3);
        assert_eq!(result.instance_chain_depth, 0);
    }

    #[test]
    fn analyze_should_return_instance_chain_depth_three_for_three_hop_chain() {
        // MI_A → MI_B → MI_C → M_Base: depth from MI_A = 3
        let graph = DependencyGraph::build(
            vec![
                node("/Game/MI_A", AssetType::MaterialInstance),
                node("/Game/MI_B", AssetType::MaterialInstance),
                node("/Game/MI_C", AssetType::MaterialInstance),
                node("/Game/M_Base", AssetType::Material),
            ],
            vec![
                (ap("/Game/MI_A"), ap("/Game/MI_B")),
                (ap("/Game/MI_B"), ap("/Game/MI_C")),
                (ap("/Game/MI_C"), ap("/Game/M_Base")),
            ],
            &[] as &[&str],
        );

        let meta = make_metadata("/Game/MI_A", AssetType::MaterialInstance, None);
        let result = analyze(&meta, &graph).expect("MaterialInstance should return Some");
        assert_eq!(result.instance_chain_depth, 3);
        assert_eq!(result.texture_sample_count, 0);
    }

    #[test]
    fn analyze_should_return_zero_texture_samples_when_metadata_has_none() {
        let graph = DependencyGraph::build(vec![], vec![], &[] as &[&str]);
        let meta = make_metadata("/Game/M_Simple", AssetType::Material, None);
        let result = analyze(&meta, &graph).expect("Material should return Some");
        assert_eq!(result.texture_sample_count, 0);
    }

    #[test]
    fn analyze_should_return_non_zero_texture_count_for_material_fixture() {
        let content_root = PathBuf::from(FIXTURES_DIR);
        let fixture = content_root.join("M_Basic.uasset");
        let scan_result = uasset_lens_scanner::scan_files(&[fixture], &content_root);
        assert!(
            !scan_result.assets.is_empty(),
            "M_Basic fixture should scan successfully"
        );

        let meta = &scan_result.assets[0];
        assert_eq!(meta.asset_type, AssetType::Material);

        let graph = DependencyGraph::build(vec![], vec![], &[] as &[&str]);
        let result = analyze(meta, &graph).expect("Material should return Some(MaterialMetrics)");
        assert!(
            result.texture_sample_count > 0,
            "M_Basic has MaterialExpressionTextureSample exports"
        );
        assert_eq!(result.instance_chain_depth, 0);
    }
}
