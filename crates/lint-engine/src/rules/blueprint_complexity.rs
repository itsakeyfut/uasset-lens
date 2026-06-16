use asset_db::AssetRecord;
use scanner::BlueprintMetrics;
use shared::AssetType;
use std::collections::HashMap;

use crate::{LintRule, LintViolation, Severity};

pub use bp_analyzer::ComplexityThresholds;

#[derive(Debug, Clone, Default)]
pub struct BlueprintComplexityRule {
    pub thresholds: ComplexityThresholds,
    pub depth_by_type: HashMap<AssetType, u32>,
}

impl LintRule for BlueprintComplexityRule {
    fn rule_id(&self) -> &'static str {
        "blueprint/complexity"
    }

    fn check(&self, asset: &AssetRecord, metrics: Option<&BlueprintMetrics>) -> Vec<LintViolation> {
        let Some(metrics) = metrics else {
            return vec![];
        };
        let effective_depth = self
            .depth_by_type
            .get(&asset.asset_type)
            .copied()
            .unwrap_or(self.thresholds.max_dependency_depth);
        let effective_thresholds = ComplexityThresholds {
            max_dependency_depth: effective_depth,
            ..self.thresholds.clone()
        };
        bp_analyzer::is_complex(metrics, &effective_thresholds)
            .into_iter()
            .map(|w| LintViolation {
                severity: Severity::Error,
                rule_id: w.rule.to_owned(),
                message: w.message,
                asset_path: asset.asset_path.clone(), // clone required: cannot move out of shared reference
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::{AssetPath, AssetType};
    use std::path::PathBuf;

    fn make_record(asset_path: &str) -> AssetRecord {
        AssetRecord {
            id: 0,
            asset_path: AssetPath::new(asset_path).unwrap(),
            file_path: PathBuf::from(format!("{asset_path}.uasset")),
            asset_type: AssetType::Texture2D,
            file_size: 0,
            last_modified: 0,
        }
    }

    fn make_record_typed(asset_path: &str, asset_type: AssetType) -> AssetRecord {
        AssetRecord {
            id: 0,
            asset_path: AssetPath::new(asset_path).unwrap(),
            file_path: PathBuf::from(format!("{asset_path}.uasset")),
            asset_type,
            file_size: 0,
            last_modified: 0,
        }
    }

    fn below_threshold_metrics() -> BlueprintMetrics {
        BlueprintMetrics {
            node_count: 5,
            event_tick_count: 0,
            cast_count: 2,
            dependency_depth: 1,
        }
    }

    #[test]
    fn blueprint_complexity_rule_should_return_no_violations_when_metrics_below_all_thresholds() {
        let rule = BlueprintComplexityRule::default();
        let asset = make_record_typed("/Game/Blueprints/BP_Player", AssetType::Blueprint);
        let metrics = below_threshold_metrics();
        assert!(rule.check(&asset, Some(&metrics)).is_empty());
    }

    #[test]
    fn blueprint_complexity_rule_should_return_violation_with_node_count_rule_when_node_count_exceeds_threshold()
     {
        let rule = BlueprintComplexityRule::default();
        let asset = make_record_typed("/Game/Blueprints/BP_Player", AssetType::Blueprint);
        let metrics = BlueprintMetrics {
            node_count: 201,
            event_tick_count: 0,
            cast_count: 0,
            dependency_depth: 0,
        };
        let violations = rule.check(&asset, Some(&metrics));
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_id, "blueprint/node-count");
        assert_eq!(violations[0].severity, Severity::Error);
    }

    #[test]
    fn blueprint_complexity_rule_should_return_violation_with_event_tick_rule_when_event_tick_count_exceeds_threshold()
     {
        let rule = BlueprintComplexityRule::default();
        let asset = make_record_typed("/Game/Blueprints/BP_Enemy", AssetType::Blueprint);
        let metrics = BlueprintMetrics {
            node_count: 0,
            event_tick_count: 2,
            cast_count: 0,
            dependency_depth: 0,
        };
        let violations = rule.check(&asset, Some(&metrics));
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_id, "blueprint/event-tick");
        assert_eq!(violations[0].severity, Severity::Error);
    }

    #[test]
    fn blueprint_complexity_rule_should_return_no_violations_when_metrics_is_none() {
        let rule = BlueprintComplexityRule::default();
        let asset = make_record("/Game/Textures/T_Rock");
        assert!(rule.check(&asset, None).is_empty());
    }

    #[test]
    fn blueprint_complexity_rule_should_use_per_type_depth_when_type_is_in_depth_by_type() {
        use shared::AssetType;
        use std::collections::HashMap;
        let rule = BlueprintComplexityRule {
            thresholds: ComplexityThresholds {
                max_dependency_depth: 20,
                ..ComplexityThresholds::default()
            },
            depth_by_type: HashMap::from([(AssetType::Blueprint, 5u32)]),
        };
        // depth 6 exceeds per-type limit of 5, but not the global limit of 20
        let asset = make_record_typed("/Game/BP_Deep", AssetType::Blueprint);
        let metrics = BlueprintMetrics {
            node_count: 0,
            event_tick_count: 0,
            cast_count: 0,
            dependency_depth: 6,
        };
        let violations = rule.check(&asset, Some(&metrics));
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_id, "blueprint/dependency-depth");
    }

    #[test]
    fn blueprint_complexity_rule_should_fall_back_to_global_threshold_when_type_not_in_depth_by_type()
     {
        use shared::AssetType;
        use std::collections::HashMap;
        let rule = BlueprintComplexityRule {
            thresholds: ComplexityThresholds {
                max_dependency_depth: 5,
                ..ComplexityThresholds::default()
            },
            // Blueprint not in map → falls back to global threshold of 5
            depth_by_type: HashMap::from([(AssetType::AnimBlueprint, 20u32)]),
        };
        let asset = make_record_typed("/Game/BP_Deep", AssetType::Blueprint);
        let metrics = BlueprintMetrics {
            node_count: 0,
            event_tick_count: 0,
            cast_count: 0,
            dependency_depth: 6,
        };
        let violations = rule.check(&asset, Some(&metrics));
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_id, "blueprint/dependency-depth");
    }

    #[test]
    fn blueprint_complexity_rule_should_not_fire_when_depth_equals_per_type_threshold() {
        use shared::AssetType;
        use std::collections::HashMap;
        let rule = BlueprintComplexityRule {
            thresholds: ComplexityThresholds::default(),
            depth_by_type: HashMap::from([(AssetType::Blueprint, 10u32)]),
        };
        let asset = make_record_typed("/Game/BP_Exact", AssetType::Blueprint);
        let metrics = BlueprintMetrics {
            node_count: 0,
            event_tick_count: 0,
            cast_count: 0,
            dependency_depth: 10, // equal to threshold → no violation
        };
        assert!(rule.check(&asset, Some(&metrics)).is_empty());
    }
}
