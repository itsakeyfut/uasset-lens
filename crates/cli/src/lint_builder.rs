use crate::config::LintConfig;
use lint_engine::LintRule;

fn parse_asset_type(s: &str) -> Option<shared::AssetType> {
    use shared::AssetType;
    match s {
        "Texture2D" => Some(AssetType::Texture2D),
        "Material" => Some(AssetType::Material),
        "StaticMesh" => Some(AssetType::StaticMesh),
        "Blueprint" => Some(AssetType::Blueprint),
        "SkeletalMesh" => Some(AssetType::SkeletalMesh),
        "AnimBlueprint" => Some(AssetType::AnimBlueprint),
        "UserWidget" => Some(AssetType::UserWidget),
        "SoundWave" => Some(AssetType::SoundWave),
        _ => None,
    }
}

pub fn build_lint_rules(cfg: &LintConfig) -> Vec<Box<dyn LintRule>> {
    let mut naming = lint_engine::NamingPrefixRule::default();
    for (type_str, prefix) in &cfg.naming_prefix {
        if let Some(asset_type) = parse_asset_type(type_str) {
            naming.prefixes.insert(asset_type, prefix.clone()); // clone required: HashMap insert takes owned String
        }
    }

    let defaults = lint_engine::ComplexityThresholds::default();
    let complexity = lint_engine::BlueprintComplexityRule {
        thresholds: lint_engine::ComplexityThresholds {
            max_node_count: cfg.blueprint_max_nodes.unwrap_or(defaults.max_node_count),
            max_event_tick_count: cfg
                .blueprint_max_event_tick
                .unwrap_or(defaults.max_event_tick_count),
            max_cast_count: cfg
                .blueprint_max_cast_count
                .unwrap_or(defaults.max_cast_count),
            max_dependency_depth: defaults.max_dependency_depth,
        },
    };

    vec![
        Box::new(naming),
        Box::new(lint_engine::TextureSizeRule::default()),
        Box::new(complexity),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LintConfig;
    use shared::AssetType;
    use std::collections::HashMap;

    #[test]
    fn build_lint_rules_should_use_defaults_when_naming_prefix_is_empty() {
        let rules = build_lint_rules(&LintConfig::default());
        let t_rock = asset_db::make_record("/Game/Textures/T_Rock", AssetType::Texture2D);
        let bp_player = asset_db::make_record("/Game/Blueprints/BP_Player", AssetType::Blueprint);
        for rule in &rules {
            assert!(rule.check(&t_rock, None).is_empty());
            assert!(rule.check(&bp_player, None).is_empty());
        }
    }

    #[test]
    fn build_lint_rules_should_apply_custom_naming_prefix_when_set_in_config() {
        let cfg = LintConfig {
            naming_prefix: HashMap::from([("Texture2D".to_owned(), "TX_".to_owned())]),
            ..LintConfig::default()
        };
        let rules = build_lint_rules(&cfg);
        let tx_rock = asset_db::make_record("/Game/Textures/TX_Rock", AssetType::Texture2D);
        let t_rock = asset_db::make_record("/Game/Textures/T_Rock", AssetType::Texture2D);
        let ok_violations: Vec<_> = rules.iter().flat_map(|r| r.check(&tx_rock, None)).collect();
        let naming_violations: Vec<_> = rules.iter().flat_map(|r| r.check(&t_rock, None)).collect();
        assert!(ok_violations.is_empty());
        assert!(!naming_violations.is_empty());
    }

    #[test]
    fn build_lint_rules_should_apply_blueprint_max_nodes_threshold_when_set_in_config() {
        let cfg = LintConfig {
            blueprint_max_nodes: Some(50),
            ..LintConfig::default()
        };
        let rules = build_lint_rules(&cfg);
        let bp = asset_db::make_record("/Game/BP_Complex", AssetType::Blueprint);
        let metrics = scanner::BlueprintMetrics {
            node_count: 51,
            event_tick_count: 0,
            cast_count: 0,
            dependency_depth: 0,
        };
        let violations: Vec<_> = rules
            .iter()
            .flat_map(|r| r.check(&bp, Some(&metrics)))
            .collect();
        assert!(!violations.is_empty());
    }

    #[test]
    fn build_lint_rules_should_ignore_unknown_asset_type_keys_in_config() {
        let cfg = LintConfig {
            naming_prefix: HashMap::from([("UnknownType".to_owned(), "X_".to_owned())]),
            ..LintConfig::default()
        };
        let rules = build_lint_rules(&cfg);
        let t_rock = asset_db::make_record("/Game/Textures/T_Rock", AssetType::Texture2D);
        let ok_violations: Vec<_> = rules.iter().flat_map(|r| r.check(&t_rock, None)).collect();
        assert!(ok_violations.is_empty());
    }
}
