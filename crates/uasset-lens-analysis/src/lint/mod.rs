use std::collections::HashMap;

use uasset_lens_asset_db::AssetRecord;
use uasset_lens_scanner::BlueprintMetrics;
use uasset_lens_shared::AssetPath;

mod rules;
pub use rules::*;

pub use crate::blueprint::ComplexityThresholds;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintViolation {
    pub severity: Severity,
    pub rule_id: String,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use uasset_lens_shared::{AssetPath, AssetType};

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

    #[test]
    fn lint_engine_with_no_rules_should_return_empty_violations_for_any_input() {
        let engine = LintEngine::new(vec![]);
        let assets = vec![
            make_record("/Game/Characters/T_Rock"),
            make_record("/Game/Environment/T_Grass"),
        ];
        assert!(engine.run(&assets, &HashMap::new()).is_empty());
    }
}
