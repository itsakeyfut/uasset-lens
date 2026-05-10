use std::collections::HashMap;
use std::path::Path;

#[derive(Default, serde::Deserialize)]
pub struct ConfigFile {
    #[serde(default)]
    pub scan: ScanConfig,
    #[serde(default)]
    pub lint: LintConfig,
}

#[derive(Default, serde::Deserialize)]
pub struct ScanConfig {
    #[serde(default)]
    pub exclude_paths: Vec<String>,
}

#[derive(Default, serde::Deserialize)]
pub struct LintConfig {
    #[serde(default)]
    pub naming_prefix: HashMap<String, String>,
    pub blueprint_max_nodes: Option<u32>,
    pub blueprint_max_event_tick: Option<u32>,
    pub blueprint_max_cast_count: Option<u32>,
}

impl LintConfig {
    pub fn build_naming_prefix_rule(&self) -> lint_engine::NamingPrefixRule {
        let mut rule = lint_engine::NamingPrefixRule::default();
        for (type_str, prefix) in &self.naming_prefix {
            if let Some(asset_type) = parse_asset_type(type_str) {
                rule.prefixes.insert(asset_type, prefix.clone()); // clone required: HashMap insert takes owned String
            }
        }
        rule
    }

    pub fn build_blueprint_complexity_rule(&self) -> lint_engine::BlueprintComplexityRule {
        let defaults = lint_engine::ComplexityThresholds::default();
        lint_engine::BlueprintComplexityRule {
            thresholds: lint_engine::ComplexityThresholds {
                max_node_count: self.blueprint_max_nodes.unwrap_or(defaults.max_node_count),
                max_event_tick_count: self
                    .blueprint_max_event_tick
                    .unwrap_or(defaults.max_event_tick_count),
                max_cast_count: self
                    .blueprint_max_cast_count
                    .unwrap_or(defaults.max_cast_count),
                max_dependency_depth: defaults.max_dependency_depth,
            },
        }
    }
}

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

pub fn load_config(project_dir: &Path) -> ConfigFile {
    let path = project_dir.join(".uasset-lens.toml");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use asset_db::AssetRecord;
    use lint_engine::LintRule;
    use shared::{AssetPath, AssetType};
    use std::path::PathBuf;

    fn make_record(asset_path: &str, asset_type: AssetType) -> AssetRecord {
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
    fn load_config_should_parse_exclude_paths_from_valid_toml() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_cfg_valid_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[scan]\nexclude_paths = [\"Content/Dev/\", \"Content/Test/\"]\n",
        )
        .unwrap();

        let config = load_config(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(
            config.scan.exclude_paths,
            vec!["Content/Dev/", "Content/Test/"]
        );
    }

    #[test]
    fn load_config_should_return_default_when_file_is_missing() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_cfg_missing_{}", std::process::id()));

        let config = load_config(&dir);

        assert!(config.scan.exclude_paths.is_empty());
    }

    #[test]
    fn load_config_should_return_default_when_toml_is_malformed() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_cfg_malformed_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".uasset-lens.toml"), "[[[[not valid toml").unwrap();

        let config = load_config(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert!(config.scan.exclude_paths.is_empty());
    }

    #[test]
    fn lint_config_should_apply_blueprint_max_nodes_threshold_when_set_in_config() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_cfg_bp_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[lint]\nblueprint_max_nodes = 50\n",
        )
        .unwrap();

        let config = load_config(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        let rule = config.lint.build_blueprint_complexity_rule();
        assert_eq!(rule.thresholds.max_node_count, 50);
    }

    #[test]
    fn lint_config_should_use_default_naming_prefixes_when_naming_prefix_is_empty() {
        let config = LintConfig::default();
        let rule = config.build_naming_prefix_rule();
        let t_rock = make_record("/Game/Textures/T_Rock", AssetType::Texture2D);
        let bp_player = make_record("/Game/Blueprints/BP_Player", AssetType::Blueprint);
        assert!(rule.check(&t_rock, None).is_empty());
        assert!(rule.check(&bp_player, None).is_empty());
    }

    #[test]
    fn lint_config_should_apply_custom_naming_prefix_when_set_in_config() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_cfg_naming_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[lint]\nnaming_prefix.Texture2D = \"TX_\"\n",
        )
        .unwrap();

        let config = load_config(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        let rule = config.lint.build_naming_prefix_rule();
        let tx_rock = make_record("/Game/Textures/TX_Rock", AssetType::Texture2D);
        let t_rock = make_record("/Game/Textures/T_Rock", AssetType::Texture2D);
        assert!(rule.check(&tx_rock, None).is_empty());
        assert!(!rule.check(&t_rock, None).is_empty());
    }
}
