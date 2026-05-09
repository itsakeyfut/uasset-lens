use dependency_graph::DependencyGraph;
use shared::{AssetPath, AssetType};

pub fn detect(graph: &DependencyGraph) -> Vec<AssetPath> {
    graph
        .nodes()
        .filter(|node| node.asset_type == AssetType::ObjectRedirector)
        .map(|node| node.path.clone()) // clone required: AssetPath is not Copy
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dependency_graph::AssetNode;

    fn node(path: &str, asset_type: AssetType) -> AssetNode {
        AssetNode {
            path: AssetPath::new(path).unwrap(),
            asset_type,
        }
    }

    #[test]
    fn detect_should_return_empty_when_graph_has_no_redirectors() {
        let graph = DependencyGraph::build(
            vec![
                node("/Game/A", AssetType::Blueprint),
                node("/Game/B", AssetType::StaticMesh),
            ],
            vec![],
        );

        assert!(detect(&graph).is_empty());
    }

    #[test]
    fn detect_should_return_all_nodes_when_all_are_redirectors() {
        let graph = DependencyGraph::build(
            vec![
                node("/Game/OldName", AssetType::ObjectRedirector),
                node("/Game/OldMesh", AssetType::ObjectRedirector),
            ],
            vec![],
        );

        let mut result: Vec<_> = detect(&graph)
            .into_iter()
            .map(|p| p.as_str().to_owned())
            .collect();
        result.sort();
        assert_eq!(result, vec!["/Game/OldMesh", "/Game/OldName"]);
    }

    #[test]
    fn detect_should_return_only_redirector_nodes_in_mixed_graph() {
        let graph = DependencyGraph::build(
            vec![
                node("/Game/BP_Player", AssetType::Blueprint),
                node("/Game/OldName", AssetType::ObjectRedirector),
                node("/Game/T_Rock", AssetType::Texture2D),
                node("/Game/OldMesh", AssetType::ObjectRedirector),
            ],
            vec![],
        );

        let mut result: Vec<_> = detect(&graph)
            .into_iter()
            .map(|p| p.as_str().to_owned())
            .collect();
        result.sort();
        assert_eq!(result, vec!["/Game/OldMesh", "/Game/OldName"]);
    }
}
