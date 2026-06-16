use uasset_lens_dependency_graph::DependencyGraph;
use uasset_lens_shared::AssetPath;

pub use uasset_lens_dependency_graph::ImpactResult;

pub fn detect(graph: &DependencyGraph, target: &AssetPath) -> ImpactResult {
    graph.find_impact(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uasset_lens_dependency_graph::AssetNode;
    use uasset_lens_shared::AssetType;

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
    fn detect_should_return_empty_when_target_has_no_referencing_assets() {
        let graph = DependencyGraph::build(vec![node("/Game/Target")], vec![], &[] as &[&str]);

        let result = detect(&graph, &ap("/Game/Target"));
        assert!(result.direct.is_empty());
        assert!(result.transitive.is_empty());
    }

    #[test]
    fn detect_should_return_direct_referencing_assets() {
        // A→Target: A directly references Target
        let graph = DependencyGraph::build(
            vec![node("/Game/Target"), node("/Game/A")],
            vec![(ap("/Game/A"), ap("/Game/Target"))],
            &[] as &[&str],
        );

        let result = detect(&graph, &ap("/Game/Target"));
        assert_eq!(result.direct, vec![ap("/Game/A")]);
        assert!(result.transitive.is_empty());
    }

    #[test]
    fn detect_should_separate_direct_and_transitive_references() {
        // C→B→Target: B is direct, C is transitive
        let graph = DependencyGraph::build(
            vec![node("/Game/Target"), node("/Game/B"), node("/Game/C")],
            vec![
                (ap("/Game/B"), ap("/Game/Target")),
                (ap("/Game/C"), ap("/Game/B")),
            ],
            &[] as &[&str],
        );

        let result = detect(&graph, &ap("/Game/Target"));
        assert_eq!(result.direct, vec![ap("/Game/B")]);
        assert_eq!(result.transitive, vec![ap("/Game/C")]);
    }
}
