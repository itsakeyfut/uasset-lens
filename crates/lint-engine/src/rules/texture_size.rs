use asset_db::AssetRecord;
use scanner::BlueprintMetrics;
use shared::AssetType;

use crate::{LintRule, LintViolation, Severity};

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

#[cfg(test)]
mod tests {
    use super::*;
    use shared::{AssetPath, AssetType};
    use std::path::PathBuf;

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
}
