use dependency_graph::DependencyGraph;
use shared::{AssetPath, is_ofpa_path};

pub fn detect(graph: &DependencyGraph) -> Vec<AssetPath> {
    graph
        .nodes()
        .filter(|node| graph.in_degree(&node.path) == 0)
        .filter(|node| !is_ofpa_path(node.path.as_str()))
        .map(|node| node.path.clone()) // clone required: AssetPath is not Copy
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dependency_graph::AssetNode;
    use shared::AssetType;

    fn node(path: &str) -> AssetNode {
        AssetNode {
            path: AssetPath::new(path).unwrap(),
            asset_type: AssetType::Blueprint,
        }
    }

    fn ap(path: &str) -> AssetPath {
        AssetPath::new(path).unwrap()
    }

    #[test]
    fn detect_should_return_all_nodes_when_graph_has_no_edges() {
        let graph = DependencyGraph::build(
            vec![node("/Game/A"), node("/Game/B"), node("/Game/C")],
            vec![],
        );

        let mut result: Vec<_> = detect(&graph)
            .into_iter()
            .map(|p| p.as_str().to_owned())
            .collect();
        result.sort();
        assert_eq!(result, vec!["/Game/A", "/Game/B", "/Game/C"]);
    }

    #[test]
    fn detect_should_return_empty_when_all_nodes_are_referenced() {
        // A→B→C: B is referenced by A, C is referenced by B; A has no incoming edges
        // but in this test we make a chain where every node is referenced by something.
        // Use a cycle so every node has in_degree >= 1.
        let graph = DependencyGraph::build(
            vec![node("/Game/A"), node("/Game/B"), node("/Game/C")],
            vec![
                (ap("/Game/A"), ap("/Game/B")),
                (ap("/Game/B"), ap("/Game/C")),
                (ap("/Game/C"), ap("/Game/A")),
            ],
        );

        assert!(detect(&graph).is_empty());
    }

    #[test]
    fn detect_should_exclude_external_actors_paths() {
        let graph = DependencyGraph::build(
            vec![
                node("/Game/A"),
                node("/Game/Maps/__ExternalActors__/MyMap/SomeActor"),
            ],
            vec![],
        );

        let result: Vec<_> = detect(&graph)
            .into_iter()
            .map(|p| p.as_str().to_owned())
            .collect();
        assert_eq!(result, vec!["/Game/A"]);
    }

    #[test]
    fn detect_should_exclude_external_objects_paths() {
        let graph = DependencyGraph::build(
            vec![
                node("/Game/A"),
                node("/Game/Maps/__ExternalObjects__/MyMap/SomeObject"),
            ],
            vec![],
        );

        let result: Vec<_> = detect(&graph)
            .into_iter()
            .map(|p| p.as_str().to_owned())
            .collect();
        assert_eq!(result, vec!["/Game/A"]);
    }

    #[test]
    fn detect_should_not_exclude_normal_assets_with_zero_in_degree() {
        let graph = DependencyGraph::build(
            vec![node("/Game/A"), node("/Game/B")],
            vec![(ap("/Game/A"), ap("/Game/B"))],
        );

        let result: Vec<_> = detect(&graph)
            .into_iter()
            .map(|p| p.as_str().to_owned())
            .collect();
        // A has in_degree 0 and is not OFPA → must appear
        assert_eq!(result, vec!["/Game/A"]);
    }

    #[test]
    fn detect_should_return_only_unreferenced_nodes_in_mixed_graph() {
        // A→B: B is referenced, A and C are not.
        let graph = DependencyGraph::build(
            vec![node("/Game/A"), node("/Game/B"), node("/Game/C")],
            vec![(ap("/Game/A"), ap("/Game/B"))],
        );

        let mut result: Vec<_> = detect(&graph)
            .into_iter()
            .map(|p| p.as_str().to_owned())
            .collect();
        result.sort();
        assert_eq!(result, vec!["/Game/A", "/Game/C"]);
    }
}
