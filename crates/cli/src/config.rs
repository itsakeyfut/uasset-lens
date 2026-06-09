use anyhow::Context;
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
    #[serde(default)]
    pub diff: DiffConfig,
}

#[derive(serde::Deserialize)]
pub struct ScanConfig {
    #[serde(default)]
    pub exclude_paths: Vec<String>,
    #[serde(default = "default_external_roots")]
    pub external_roots: Vec<String>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            exclude_paths: Vec::new(),
            external_roots: default_external_roots(),
        }
    }
}

pub(crate) fn default_external_roots() -> Vec<String> {
    vec!["/Engine/".to_string(), "/Script/".to_string()]
}

#[derive(Default, serde::Deserialize)]
pub struct LintConfig {
    #[serde(default)]
    pub naming_prefix: HashMap<String, String>,
    pub blueprint_max_nodes: Option<u32>,
    pub blueprint_max_event_tick: Option<u32>,
    pub blueprint_max_cast_count: Option<u32>,
    pub blueprint_max_dependency_depth: Option<u32>,
    #[serde(default)]
    pub blueprint_depth_by_type: HashMap<String, u32>,
}

#[derive(serde::Deserialize)]
pub struct DiffConfig {
    #[serde(default = "default_size_increase_threshold_pct")]
    pub size_increase_threshold_pct: u64,
}

impl Default for DiffConfig {
    fn default() -> Self {
        Self {
            size_increase_threshold_pct: default_size_increase_threshold_pct(),
        }
    }
}

fn default_size_increase_threshold_pct() -> u64 {
    10
}

pub fn load_config(project_dir: &Path) -> ConfigFile {
    load_config_at(&project_dir.join(".uasset-lens.toml"))
}

pub fn load_config_at(path: &Path) -> ConfigFile {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn resolve_config(
    project_dir: &Path,
    explicit: Option<&Path>,
) -> anyhow::Result<ConfigFile> {
    match explicit {
        Some(path) => {
            let s = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read config file: {}", path.display()))?;
            toml::from_str(&s)
                .with_context(|| format!("invalid TOML in config file: {}", path.display()))
        }
        None => Ok(load_config_at(&project_dir.join(".uasset-lens.toml"))),
    }
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
    fn load_config_should_parse_diff_threshold_from_valid_toml() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_cfg_diff_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[diff]\nsize_increase_threshold_pct = 25\n",
        )
        .unwrap();

        let config = load_config(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(config.diff.size_increase_threshold_pct, 25);
    }

    #[test]
    fn load_config_should_use_default_diff_threshold_when_not_set() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_cfg_diff_dflt_{}", std::process::id()));

        let config = load_config(&dir);

        assert_eq!(config.diff.size_increase_threshold_pct, 10);
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

    #[test]
    fn load_config_should_parse_external_roots_from_valid_toml() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_cfg_roots_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[scan]\nexternal_roots = [\"/Engine/\", \"/Plugins/\"]\n",
        )
        .unwrap();

        let config = load_config(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(config.scan.external_roots, vec!["/Engine/", "/Plugins/"]);
    }

    #[test]
    fn load_config_should_use_default_external_roots_when_key_absent() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_cfg_roots_dflt_{}", std::process::id()));

        let config = load_config(&dir);

        assert_eq!(
            config.scan.external_roots,
            vec!["/Engine/", "/Script/"],
            "default external roots should be /Engine/ and /Script/"
        );
    }

    #[test]
    fn load_config_at_should_load_config_from_explicit_path() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_at_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.toml");
        std::fs::write(&path, "[lint]\nblueprint_max_dependency_depth = 5\n").unwrap();
        let cfg = load_config_at(&path);
        assert_eq!(cfg.lint.blueprint_max_dependency_depth, Some(5));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_config_at_should_return_default_when_file_is_missing() {
        let path = std::env::temp_dir()
            .join(format!("uasset_lens_absent_{}.toml", std::process::id()));
        // don't create it — we want it to be missing
        let cfg = load_config_at(&path);
        assert!(cfg.lint.blueprint_max_dependency_depth.is_none());
        assert!(cfg.lint.blueprint_depth_by_type.is_empty());
    }

    #[test]
    fn resolve_config_should_load_from_explicit_path_when_specified() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_res_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("custom.toml");
        std::fs::write(&path, "[lint]\nblueprint_max_dependency_depth = 7\n").unwrap();
        let cfg = resolve_config(std::path::Path::new("."), Some(&path)).unwrap();
        assert_eq!(cfg.lint.blueprint_max_dependency_depth, Some(7));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_config_should_return_error_when_explicit_path_is_missing() {
        let absent =
            std::env::temp_dir().join(format!("uasset_lens_absent_err_{}.toml", std::process::id()));
        let result = resolve_config(std::path::Path::new("."), Some(&absent));
        assert!(result.is_err());
    }

    #[test]
    fn resolve_config_should_use_project_dir_default_when_no_explicit_path() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_def_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[lint]\nblueprint_max_dependency_depth = 3\n",
        )
        .unwrap();
        let cfg = resolve_config(&dir, None).unwrap();
        assert_eq!(cfg.lint.blueprint_max_dependency_depth, Some(3));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_config_should_parse_blueprint_max_dependency_depth() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_bdd_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[lint]\nblueprint_max_dependency_depth = 12\n",
        )
        .unwrap();
        let cfg = load_config(&dir);
        assert_eq!(cfg.lint.blueprint_max_dependency_depth, Some(12));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_config_should_parse_blueprint_depth_by_type() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_dbt_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[lint.blueprint_depth_by_type]\nBlueprint = 10\nAnimBlueprint = 20\n",
        )
        .unwrap();
        let cfg = load_config(&dir);
        assert_eq!(cfg.lint.blueprint_depth_by_type.get("Blueprint"), Some(&10u32));
        assert_eq!(cfg.lint.blueprint_depth_by_type.get("AnimBlueprint"), Some(&20u32));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
