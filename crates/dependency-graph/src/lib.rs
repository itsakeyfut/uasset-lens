use std::collections::HashMap;

use petgraph::Direction;
use petgraph::graph::{DiGraph, NodeIndex};
use shared::{AssetPath, AssetType};

pub struct AssetNode {
    pub path: AssetPath,
    pub asset_type: AssetType,
}

/// Returned by `find_impact()` (implemented in a later issue).
pub struct ImpactResult {
    pub direct: Vec<AssetPath>,
    pub transitive: Vec<AssetPath>,
}

pub struct DependencyGraph {
    graph: DiGraph<AssetNode, ()>,
    index: HashMap<AssetPath, NodeIndex>,
}

impl DependencyGraph {
    pub fn build(
        nodes: impl IntoIterator<Item = AssetNode>,
        edges: impl IntoIterator<Item = (AssetPath, AssetPath)>,
    ) -> Self {
        let mut graph = DiGraph::new();
        let mut index: HashMap<AssetPath, NodeIndex> = HashMap::new();

        for node in nodes {
            let path = node.path.clone(); // clone required: path is moved into graph node
            let idx = graph.add_node(node);
            index.insert(path, idx);
        }

        for (from_path, to_path) in edges {
            let from_idx = get_or_insert_placeholder(&mut graph, &mut index, from_path);
            let to_idx = get_or_insert_placeholder(&mut graph, &mut index, to_path);
            graph.add_edge(from_idx, to_idx, ());
        }

        Self { graph, index }
    }

    pub fn nodes(&self) -> impl Iterator<Item = &AssetNode> {
        self.graph.node_weights()
    }

    pub fn in_degree(&self, path: &AssetPath) -> usize {
        self.index
            .get(path)
            .map(|&idx| self.graph.edges_directed(idx, Direction::Incoming).count())
            .unwrap_or(0)
    }
}

fn get_or_insert_placeholder(
    graph: &mut DiGraph<AssetNode, ()>,
    index: &mut HashMap<AssetPath, NodeIndex>,
    path: AssetPath,
) -> NodeIndex {
    if let Some(&idx) = index.get(&path) {
        return idx;
    }
    let idx = graph.add_node(AssetNode {
        path: path.clone(), // clone required: path is moved into hashmap key below
        asset_type: AssetType::Unknown("".into()),
    });
    index.insert(path, idx);
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn build_should_contain_all_nodes_when_built_without_edges() {
        let graph = DependencyGraph::build(
            vec![node("/Game/A"), node("/Game/B"), node("/Game/C")],
            vec![],
        );

        let paths: Vec<_> = graph.nodes().map(|n| n.path.as_str()).collect();
        assert!(paths.contains(&"/Game/A"));
        assert!(paths.contains(&"/Game/B"));
        assert!(paths.contains(&"/Game/C"));
        assert_eq!(paths.len(), 3);
    }

    #[test]
    fn build_should_create_placeholder_node_for_unknown_to_path() {
        let graph = DependencyGraph::build(
            vec![node("/Game/A")],
            vec![(ap("/Game/A"), ap("/Game/Unknown"))],
        );

        let paths: Vec<_> = graph.nodes().map(|n| n.path.as_str()).collect();
        assert!(
            paths.contains(&"/Game/Unknown"),
            "placeholder node should be created for unknown to_path"
        );
        assert_eq!(paths.len(), 2);

        let placeholder = graph
            .nodes()
            .find(|n| n.path.as_str() == "/Game/Unknown")
            .unwrap();
        assert_eq!(
            placeholder.asset_type,
            AssetType::Unknown("".into()),
            "placeholder should have Unknown asset type with empty string"
        );
    }

    #[test]
    fn build_should_create_placeholder_node_for_unknown_from_path() {
        let graph = DependencyGraph::build(
            vec![node("/Game/B")],
            vec![(ap("/Game/Unknown"), ap("/Game/B"))],
        );

        let paths: Vec<_> = graph.nodes().map(|n| n.path.as_str()).collect();
        assert!(
            paths.contains(&"/Game/Unknown"),
            "placeholder node should be created for unknown from_path"
        );
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn nodes_should_return_all_nodes_including_isolated_ones() {
        let graph = DependencyGraph::build(
            vec![node("/Game/A"), node("/Game/B"), node("/Game/C")],
            vec![(ap("/Game/A"), ap("/Game/B"))],
        );

        let paths: Vec<_> = graph.nodes().map(|n| n.path.as_str()).collect();
        assert!(paths.contains(&"/Game/A"));
        assert!(paths.contains(&"/Game/B"));
        assert!(
            paths.contains(&"/Game/C"),
            "isolated node /Game/C should also be returned"
        );
        assert_eq!(paths.len(), 3);
    }

    #[test]
    fn in_degree_should_return_zero_for_unreferenced_node() {
        let graph = DependencyGraph::build(
            vec![node("/Game/A"), node("/Game/B")],
            vec![(ap("/Game/A"), ap("/Game/B"))],
        );

        assert_eq!(
            graph.in_degree(&ap("/Game/A")),
            0,
            "/Game/A is not referenced by anything"
        );
    }

    #[test]
    fn in_degree_should_return_correct_count_for_multiply_referenced_node() {
        let graph = DependencyGraph::build(
            vec![
                node("/Game/A"),
                node("/Game/B"),
                node("/Game/C"),
                node("/Game/D"),
            ],
            vec![
                (ap("/Game/A"), ap("/Game/D")),
                (ap("/Game/B"), ap("/Game/D")),
                (ap("/Game/C"), ap("/Game/D")),
            ],
        );

        assert_eq!(
            graph.in_degree(&ap("/Game/D")),
            3,
            "/Game/D is referenced by A, B, and C"
        );
    }

    #[test]
    fn in_degree_should_return_zero_for_unknown_path() {
        let graph = DependencyGraph::build(vec![node("/Game/A")], vec![]);

        assert_eq!(
            graph.in_degree(&ap("/Game/NotInGraph")),
            0,
            "unknown path should return 0"
        );
    }
}
