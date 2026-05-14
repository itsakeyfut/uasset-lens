use asset_db::AssetRecord;
use scanner::BlueprintMetrics;

use crate::{LintRule, LintViolation, Severity};

pub use bp_analyzer::ComplexityThresholds;

#[derive(Debug, Clone, Default)]
pub struct BlueprintComplexityRule {
    pub thresholds: ComplexityThresholds,
}

impl LintRule for BlueprintComplexityRule {
    fn rule_id(&self) -> &'static str {
        "blueprint/complexity"
    }

    fn check(&self, asset: &AssetRecord, metrics: Option<&BlueprintMetrics>) -> Vec<LintViolation> {
        let Some(metrics) = metrics else {
            return vec![];
        };
        bp_analyzer::is_complex(metrics, &self.thresholds)
            .into_iter()
            .map(|w| LintViolation {
                severity: Severity::Error,
                rule_id: w.rule,
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
}
