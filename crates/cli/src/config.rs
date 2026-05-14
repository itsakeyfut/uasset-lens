use std::collections::HashMap;
use std::path::Path;

#[derive(Default, serde::Deserialize)]
pub struct ConfigFile {
    #[serde(default)]
    pub scan: ScanConfig,
    #[serde(default)]
    pub lint: LintConfig,
    #[serde(default)]
    pub budget: budget_tracker::BudgetConfig,
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
    fn load_config_should_parse_budget_limits_from_valid_toml() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_cfg_budget_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[budget]\nTexture2D.max_size = 4194304\nSoundWave.max_size = 2097152\n",
        )
        .unwrap();

        let config = load_config(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(
            config.budget.limits.get("Texture2D").map(|b| b.max_size),
            Some(4194304)
        );
        assert_eq!(
            config.budget.limits.get("SoundWave").map(|b| b.max_size),
            Some(2097152)
        );
    }
}
