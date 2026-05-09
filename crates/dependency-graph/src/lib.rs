use std::collections::{HashMap, HashSet, VecDeque};

use petgraph::Direction;
use petgraph::graph::{DiGraph, NodeIndex};
use shared::{AssetPath, AssetType};

pub struct AssetNode {
    pub path: AssetPath,
    pub asset_type: AssetType,
}

/// Assets that would be affected if the target asset were deleted or renamed.
/// `direct` and `transitive` are mutually exclusive; neither contains the target itself.
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

    pub fn find_impact(&self, target: &AssetPath) -> ImpactResult {
        let start_idx = match self.index.get(target) {
            Some(&idx) => idx,
            None => {
                return ImpactResult {
                    direct: vec![],
                    transitive: vec![],
                };
            }
        };

        let mut direct = Vec::new();
        let mut transitive = Vec::new();
        let mut visited: HashSet<NodeIndex> = HashSet::new();
        visited.insert(start_idx);

        // BFS on incoming edges (who references target) with depth tracking.
        // Depth 1 → direct; depth 2+ → transitive.
        let mut queue: VecDeque<(NodeIndex, usize)> = VecDeque::new();
        for neighbor in self
            .graph
            .neighbors_directed(start_idx, Direction::Incoming)
        {
            if visited.insert(neighbor) {
                queue.push_back((neighbor, 1));
            }
        }

        while let Some((node_idx, depth)) = queue.pop_front() {
            let path = self.graph[node_idx].path.clone(); // clone required: AssetPath is not Copy
            if depth == 1 {
                direct.push(path);
            } else {
                transitive.push(path);
            }
            for neighbor in self.graph.neighbors_directed(node_idx, Direction::Incoming) {
                if visited.insert(neighbor) {
                    queue.push_back((neighbor, depth + 1));
                }
            }
        }

        ImpactResult { direct, transitive }
    }

    pub fn find_cycles(&self) -> Vec<Vec<AssetPath>> {
        petgraph::algo::tarjan_scc(&self.graph)
            .into_iter()
            .filter(|scc| scc.len() >= 2)
            .map(|scc| {
                scc.into_iter()
                    .map(|idx| self.graph[idx].path.clone()) // clone required: AssetPath is not Copy
                    .collect()
            })
            .collect()
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

    #[test]
    fn find_impact_should_return_direct_only_when_single_hop_referencing_assets() {
        // B→A and C→A: both directly reference A, no transitive chain.
        let graph = DependencyGraph::build(
            vec![node("/Game/A"), node("/Game/B"), node("/Game/C")],
            vec![
                (ap("/Game/B"), ap("/Game/A")),
                (ap("/Game/C"), ap("/Game/A")),
            ],
        );

        let result = graph.find_impact(&ap("/Game/A"));
        assert_eq!(result.direct.len(), 2);
        assert!(result.direct.iter().any(|p| p.as_str() == "/Game/B"));
        assert!(result.direct.iter().any(|p| p.as_str() == "/Game/C"));
        assert!(result.transitive.is_empty());
    }

    #[test]
    fn find_impact_should_separate_direct_and_transitive_for_multi_hop_chain() {
        // C→B→A: B directly references A; C references A only through B.
        let graph = DependencyGraph::build(
            vec![node("/Game/A"), node("/Game/B"), node("/Game/C")],
            vec![
                (ap("/Game/B"), ap("/Game/A")),
                (ap("/Game/C"), ap("/Game/B")),
            ],
        );

        let result = graph.find_impact(&ap("/Game/A"));
        assert_eq!(result.direct, vec![ap("/Game/B")]);
        assert_eq!(result.transitive, vec![ap("/Game/C")]);
    }

    #[test]
    fn find_impact_should_return_empty_when_no_referencing_assets() {
        let graph = DependencyGraph::build(
            vec![node("/Game/A"), node("/Game/B")],
            vec![(ap("/Game/A"), ap("/Game/B"))],
        );

        let result = graph.find_impact(&ap("/Game/A"));
        assert!(result.direct.is_empty());
        assert!(result.transitive.is_empty());
    }

    #[test]
    fn find_impact_should_return_empty_for_target_not_in_graph() {
        let graph = DependencyGraph::build(vec![node("/Game/A")], vec![]);

        let result = graph.find_impact(&ap("/Game/NotInGraph"));
        assert!(result.direct.is_empty());
        assert!(result.transitive.is_empty());
    }

    #[test]
    fn find_impact_should_not_include_target_in_results() {
        let graph = DependencyGraph::build(
            vec![node("/Game/A"), node("/Game/B")],
            vec![(ap("/Game/B"), ap("/Game/A"))],
        );

        let result = graph.find_impact(&ap("/Game/A"));
        let all_paths: Vec<_> = result
            .direct
            .iter()
            .chain(result.transitive.iter())
            .collect();
        assert!(!all_paths.iter().any(|p| p.as_str() == "/Game/A"));
    }

    #[test]
    fn find_cycles_should_return_empty_for_dag() {
        let graph = DependencyGraph::build(
            vec![node("/Game/A"), node("/Game/B"), node("/Game/C")],
            vec![
                (ap("/Game/A"), ap("/Game/B")),
                (ap("/Game/B"), ap("/Game/C")),
            ],
        );

        assert!(graph.find_cycles().is_empty());
    }

    #[test]
    fn find_cycles_should_return_one_cycle_for_two_node_mutual_reference() {
        let graph = DependencyGraph::build(
            vec![node("/Game/A"), node("/Game/B")],
            vec![
                (ap("/Game/A"), ap("/Game/B")),
                (ap("/Game/B"), ap("/Game/A")),
            ],
        );

        let cycles = graph.find_cycles();
        assert_eq!(cycles.len(), 1);
        let cycle = &cycles[0];
        assert_eq!(cycle.len(), 2);
        assert!(cycle.iter().any(|p| p.as_str() == "/Game/A"));
        assert!(cycle.iter().any(|p| p.as_str() == "/Game/B"));
    }

    #[test]
    fn find_cycles_should_return_one_cycle_for_three_node_cycle() {
        let graph = DependencyGraph::build(
            vec![node("/Game/A"), node("/Game/B"), node("/Game/C")],
            vec![
                (ap("/Game/A"), ap("/Game/B")),
                (ap("/Game/B"), ap("/Game/C")),
                (ap("/Game/C"), ap("/Game/A")),
            ],
        );

        let cycles = graph.find_cycles();
        assert_eq!(cycles.len(), 1);
        let cycle = &cycles[0];
        assert_eq!(cycle.len(), 3);
        assert!(cycle.iter().any(|p| p.as_str() == "/Game/A"));
        assert!(cycle.iter().any(|p| p.as_str() == "/Game/B"));
        assert!(cycle.iter().any(|p| p.as_str() == "/Game/C"));
    }

    #[test]
    fn find_cycles_should_return_two_entries_for_independent_cycles() {
        let graph = DependencyGraph::build(
            vec![
                node("/Game/A"),
                node("/Game/B"),
                node("/Game/C"),
                node("/Game/D"),
            ],
            vec![
                (ap("/Game/A"), ap("/Game/B")),
                (ap("/Game/B"), ap("/Game/A")),
                (ap("/Game/C"), ap("/Game/D")),
                (ap("/Game/D"), ap("/Game/C")),
            ],
        );

        let cycles = graph.find_cycles();
        assert_eq!(cycles.len(), 2, "two independent cycles should be detected");
        assert!(cycles.iter().all(|c| c.len() == 2));
    }

    #[test]
    fn find_cycles_should_exclude_self_loop_only() {
        // A self-loop A→A forms a single-node SCC, which must be filtered out.
        let graph =
            DependencyGraph::build(vec![node("/Game/A")], vec![(ap("/Game/A"), ap("/Game/A"))]);

        assert!(
            graph.find_cycles().is_empty(),
            "self-loop should not be reported as a cycle"
        );
    }
}
