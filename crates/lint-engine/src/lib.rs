use std::collections::HashMap;

use asset_db::AssetRecord;
use scanner::BlueprintMetrics;
use shared::{AssetPath, AssetType};

pub use bp_analyzer::ComplexityThresholds;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintViolation {
    pub severity: Severity,
    pub rule_id: &'static str,
    pub message: String,
    pub asset_path: AssetPath,
}

pub trait LintRule: Send + Sync {
    fn rule_id(&self) -> &'static str;
    fn check(&self, asset: &AssetRecord, metrics: Option<&BlueprintMetrics>) -> Vec<LintViolation>;
}

pub struct LintEngine {
    rules: Vec<Box<dyn LintRule>>,
}

impl LintEngine {
    pub fn new(rules: Vec<Box<dyn LintRule>>) -> Self {
        Self { rules }
    }

    pub fn run(
        &self,
        assets: &[AssetRecord],
        metrics_map: &HashMap<AssetPath, BlueprintMetrics>,
    ) -> Vec<LintViolation> {
        let mut violations = Vec::new();
        for asset in assets {
            let metrics = metrics_map.get(&asset.asset_path);
            for rule in &self.rules {
                violations.extend(rule.check(asset, metrics));
            }
        }
        violations
    }
}

#[derive(Debug, Clone)]
pub struct NamingPrefixRule {
    pub prefixes: HashMap<AssetType, String>,
}

impl Default for NamingPrefixRule {
    fn default() -> Self {
        let mut prefixes = HashMap::new();
        prefixes.insert(AssetType::Texture2D, "T_".to_owned());
        prefixes.insert(AssetType::Material, "M_".to_owned());
        prefixes.insert(AssetType::StaticMesh, "SM_".to_owned());
        prefixes.insert(AssetType::Blueprint, "BP_".to_owned());
        prefixes.insert(AssetType::SkeletalMesh, "SK_".to_owned());
        prefixes.insert(AssetType::AnimBlueprint, "ABP_".to_owned());
        prefixes.insert(AssetType::UserWidget, "WBP_".to_owned());
        prefixes.insert(AssetType::SoundWave, "S_".to_owned());
        Self { prefixes }
    }
}

impl LintRule for NamingPrefixRule {
    fn rule_id(&self) -> &'static str {
        "naming/prefix"
    }

    fn check(
        &self,
        asset: &AssetRecord,
        _metrics: Option<&BlueprintMetrics>,
    ) -> Vec<LintViolation> {
        if matches!(asset.asset_type, AssetType::Unknown(_)) {
            return vec![];
        }
        let Some(expected_prefix) = self.prefixes.get(&asset.asset_type) else {
            return vec![];
        };
        let path = asset.asset_path.as_str();
        // AssetPath always starts with '/', so rsplit always yields at least one element
        let name = path.rsplit('/').next().unwrap_or(path);
        if name.starts_with(expected_prefix.as_str()) {
            return vec![];
        }
        vec![LintViolation {
            severity: Severity::Warning,
            rule_id: "naming/prefix",
            message: format!("asset `{name}` should start with prefix `{expected_prefix}`"),
            asset_path: asset.asset_path.clone(), // clone required: cannot move out of shared reference
        }]
    }
}

#[derive(Debug, Clone)]
pub struct TextureSizeRule {
    pub max_size: u64,
}

impl Default for TextureSizeRule {
    fn default() -> Self {
        Self {
            max_size: 4 * 1024 * 1024,
        }
    }
}

impl LintRule for TextureSizeRule {
    fn rule_id(&self) -> &'static str {
        "budget/texture-size"
    }

    fn check(
        &self,
        asset: &AssetRecord,
        _metrics: Option<&BlueprintMetrics>,
    ) -> Vec<LintViolation> {
        if asset.asset_type != AssetType::Texture2D {
            return vec![];
        }
        if asset.file_size <= self.max_size {
            return vec![];
        }
        let path = asset.asset_path.as_str();
        // AssetPath always starts with '/', so rsplit always yields at least one element
        let name = path.rsplit('/').next().unwrap_or(path);
        let actual_mb = asset.file_size as f64 / (1024.0 * 1024.0);
        let max_mb = self.max_size as f64 / (1024.0 * 1024.0);
        vec![LintViolation {
            severity: Severity::Warning,
            rule_id: "budget/texture-size",
            message: format!(
                "Texture2D {name} exceeds size limit: {actual_mb:.1} MB > {max_mb:.1} MB"
            ),
            asset_path: asset.asset_path.clone(), // clone required: cannot move out of shared reference
        }]
    }
}

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

    fn make_record_with_size(
        asset_path: &str,
        asset_type: AssetType,
        file_size: u64,
    ) -> AssetRecord {
        AssetRecord {
            id: 0,
            asset_path: AssetPath::new(asset_path).unwrap(),
            file_path: PathBuf::from(format!("{asset_path}.uasset")),
            asset_type,
            file_size,
            last_modified: 0,
        }
    }

    #[test]
    fn lint_engine_with_no_rules_should_return_empty_violations_for_any_input() {
        let engine = LintEngine::new(vec![]);
        let assets = vec![
            make_record("/Game/Characters/T_Rock"),
            make_record("/Game/Environment/T_Grass"),
        ];
        assert!(engine.run(&assets, &HashMap::new()).is_empty());
    }

    #[test]
    fn naming_prefix_rule_should_not_emit_violation_for_correctly_prefixed_texture() {
        let rule = NamingPrefixRule::default();
        let asset = make_record("/Game/Textures/T_Rock");
        assert!(rule.check(&asset, None).is_empty());
    }

    #[test]
    fn naming_prefix_rule_should_emit_violation_for_texture_without_prefix() {
        let rule = NamingPrefixRule::default();
        let asset = make_record("/Game/Textures/Rock");
        let violations = rule.check(&asset, None);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_id, "naming/prefix");
        assert_eq!(violations[0].severity, Severity::Warning);
    }

    #[test]
    fn naming_prefix_rule_should_skip_unknown_asset_type() {
        let rule = NamingPrefixRule::default();
        let asset = make_record_typed(
            "/Game/Misc/SomeAsset",
            AssetType::Unknown("SomeClass".to_owned()),
        );
        assert!(rule.check(&asset, None).is_empty());
    }

    #[test]
    fn naming_prefix_rule_should_not_emit_violation_for_type_not_in_prefix_map() {
        let rule = NamingPrefixRule::default();
        let asset = make_record_typed("/Game/Maps/MyLevel", AssetType::World);
        assert!(rule.check(&asset, None).is_empty());
    }

    #[test]
    fn texture_size_rule_should_not_emit_violation_when_size_equals_max() {
        let rule = TextureSizeRule::default();
        let asset = make_record_with_size(
            "/Game/Textures/T_Rock",
            AssetType::Texture2D,
            4 * 1024 * 1024,
        );
        assert!(rule.check(&asset, None).is_empty());
    }

    #[test]
    fn texture_size_rule_should_emit_violation_when_size_exceeds_max_by_one_byte() {
        let rule = TextureSizeRule::default();
        let asset = make_record_with_size(
            "/Game/Textures/T_Rock",
            AssetType::Texture2D,
            4 * 1024 * 1024 + 1,
        );
        let violations = rule.check(&asset, None);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_id, "budget/texture-size");
        assert_eq!(violations[0].severity, Severity::Warning);
    }

    #[test]
    fn texture_size_rule_should_not_emit_violation_for_static_mesh_over_max() {
        let rule = TextureSizeRule::default();
        let asset = make_record_with_size(
            "/Game/Meshes/SM_Rock",
            AssetType::StaticMesh,
            4 * 1024 * 1024 + 1,
        );
        assert!(rule.check(&asset, None).is_empty());
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
