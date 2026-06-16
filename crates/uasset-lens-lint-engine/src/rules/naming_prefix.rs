use std::collections::HashMap;

use uasset_lens_asset_db::AssetRecord;
use uasset_lens_scanner::BlueprintMetrics;
use uasset_lens_shared::AssetType;

use crate::{LintRule, LintViolation, Severity};

/// Maps each asset type to one or more valid name prefixes.
/// A name is accepted if it starts with ANY entry in the Vec.
/// Config overrides replace the entire Vec for a given type.
#[derive(Debug, Clone)]
pub struct NamingPrefixRule {
    pub prefixes: HashMap<AssetType, Vec<String>>,
}

impl Default for NamingPrefixRule {
    fn default() -> Self {
        let mut prefixes: HashMap<AssetType, Vec<String>> = HashMap::new();
        prefixes.insert(AssetType::Texture2D, vec!["T_".to_owned()]);
        prefixes.insert(AssetType::Material, vec!["M_".to_owned()]);
        prefixes.insert(AssetType::StaticMesh, vec!["SM_".to_owned()]);
        prefixes.insert(AssetType::Blueprint, vec!["BP_".to_owned()]);
        // UE5 uses SKM_ for SkeletalMesh; older UE4 projects used SK_
        prefixes.insert(
            AssetType::SkeletalMesh,
            vec!["SKM_".to_owned(), "SK_".to_owned()],
        );
        prefixes.insert(AssetType::AnimBlueprint, vec!["ABP_".to_owned()]);
        prefixes.insert(AssetType::UserWidget, vec!["WBP_".to_owned()]);
        // UE5 convention is SW_ for SoundWave (S_ was UE4)
        prefixes.insert(
            AssetType::SoundWave,
            vec!["SW_".to_owned(), "S_".to_owned()],
        );
        prefixes.insert(AssetType::NiagaraSystem, vec!["NS_".to_owned()]);
        // Many projects use NS_ for standalone emitters too; NE_ is the strict convention
        prefixes.insert(
            AssetType::NiagaraEmitter,
            vec!["NE_".to_owned(), "NS_".to_owned()],
        );
        // IKRigDefinition (IKR_), IKRetargeter (RETG_), DialogueWave (DW_) have no default
        // prefix rule: naming conventions vary widely across UE5 projects and Epic's own samples
        // are inconsistent. Users can configure these via .uasset-lens.toml if desired.
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
        let Some(valid_prefixes) = self.prefixes.get(&asset.asset_type) else {
            return vec![];
        };
        let path = asset.asset_path.as_str();
        // AssetPath always starts with '/', so rsplit always yields at least one element
        let name = path.rsplit('/').next().unwrap_or(path);
        if valid_prefixes.iter().any(|p| name.starts_with(p.as_str())) {
            return vec![];
        }
        let expected = if valid_prefixes.len() == 1 {
            format!("`{}`", valid_prefixes[0])
        } else {
            let joined = valid_prefixes
                .iter()
                .map(|p| format!("`{p}`"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("one of: {joined}")
        };
        vec![LintViolation {
            severity: Severity::Warning,
            rule_id: "naming/prefix".to_owned(),
            message: format!("asset `{name}` should start with {expected}"),
            asset_path: asset.asset_path.clone(), // clone required: cannot move out of shared reference
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uasset_lens_shared::{AssetPath, AssetType};
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
    fn naming_prefix_rule_should_accept_skm_prefix_for_skeletal_mesh() {
        let rule = NamingPrefixRule::default();
        let asset = make_record_typed("/Game/Characters/SKM_Manny", AssetType::SkeletalMesh);
        assert!(rule.check(&asset, None).is_empty());
    }

    #[test]
    fn naming_prefix_rule_should_accept_sk_prefix_for_skeletal_mesh() {
        let rule = NamingPrefixRule::default();
        let asset = make_record_typed("/Game/Characters/SK_Hero", AssetType::SkeletalMesh);
        assert!(rule.check(&asset, None).is_empty());
    }

    #[test]
    fn naming_prefix_rule_should_accept_sw_prefix_for_sound_wave() {
        let rule = NamingPrefixRule::default();
        let asset = make_record_typed("/Game/Sounds/SW_Explosion", AssetType::SoundWave);
        assert!(rule.check(&asset, None).is_empty());
    }

    #[test]
    fn naming_prefix_rule_should_accept_s_prefix_for_sound_wave() {
        let rule = NamingPrefixRule::default();
        let asset = make_record_typed("/Game/Sounds/S_Ambient", AssetType::SoundWave);
        assert!(rule.check(&asset, None).is_empty());
    }

    #[test]
    fn naming_prefix_rule_should_accept_ns_prefix_for_niagara_system() {
        let rule = NamingPrefixRule::default();
        let asset = make_record_typed("/Game/Niagara/NS_Fire", AssetType::NiagaraSystem);
        assert!(rule.check(&asset, None).is_empty());
    }

    #[test]
    fn naming_prefix_rule_should_emit_violation_for_niagara_system_without_prefix() {
        let rule = NamingPrefixRule::default();
        let asset = make_record_typed("/Game/Niagara/Fire", AssetType::NiagaraSystem);
        let violations = rule.check(&asset, None);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_id, "naming/prefix");
    }
}
