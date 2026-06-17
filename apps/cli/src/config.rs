use anyhow::Context;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Default, serde::Deserialize)]
pub struct ConfigFile {
    #[serde(default)]
    pub scan: ScanConfig,
    #[serde(default)]
    pub rules: RulesConfig,
    #[serde(default)]
    pub lint: LintConfig,
    #[serde(default)]
    pub budget: uasset_lens_analysis::BudgetConfig,
    #[serde(default)]
    pub diff: DiffConfig,
    #[serde(default)]
    pub check: CheckSectionConfig,
}

/// Per-check severity. `Off` disables the check; `Warn` reports findings without
/// failing CI; `Error` fails CI (exit 1) when findings exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckSeverity {
    Error,
    Warn,
    Off,
}

/// `[rules]` — severity of each `check` health check. A `None` field (key absent)
/// means "use the built-in default for that check" (see `check::effective_severity`).
#[derive(Default, serde::Deserialize)]
pub struct RulesConfig {
    #[serde(rename = "dead-assets", default)]
    pub dead_assets: Option<CheckSeverity>,
    #[serde(rename = "circular-deps", default)]
    pub circular_deps: Option<CheckSeverity>,
    #[serde(rename = "duplicate-assets", default)]
    pub duplicate_assets: Option<CheckSeverity>,
    #[serde(default)]
    pub redirectors: Option<CheckSeverity>,
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
    pub naming: LintNamingConfig,
    #[serde(default)]
    pub blueprint: LintBlueprintConfig,
}

fn default_true() -> bool {
    true
}

/// `[lint.naming]` — naming-prefix conventions. Each `*_prefix` overrides the
/// built-in default prefix for that asset type.
#[derive(serde::Deserialize)]
pub struct LintNamingConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub severity: Option<CheckSeverity>,
    pub texture_prefix: Option<String>,
    pub material_prefix: Option<String>,
    pub material_function_prefix: Option<String>,
    pub static_mesh_prefix: Option<String>,
    pub skeletal_mesh_prefix: Option<String>,
    pub blueprint_prefix: Option<String>,
    pub widget_prefix: Option<String>,
    pub anim_bp_prefix: Option<String>,
    pub sound_prefix: Option<String>,
}

impl Default for LintNamingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            severity: None,
            texture_prefix: None,
            material_prefix: None,
            material_function_prefix: None,
            static_mesh_prefix: None,
            skeletal_mesh_prefix: None,
            blueprint_prefix: None,
            widget_prefix: None,
            anim_bp_prefix: None,
            sound_prefix: None,
        }
    }
}

/// `[lint.blueprint]` — Blueprint complexity thresholds. `depth_by_type` overrides
/// the dependency-depth limit per asset type (extension beyond the documented limits).
#[derive(serde::Deserialize)]
pub struct LintBlueprintConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub severity: Option<CheckSeverity>,
    pub event_tick_limit: Option<u32>,
    pub cast_limit: Option<u32>,
    pub node_limit: Option<u32>,
    pub dependency_depth_limit: Option<u32>,
    #[serde(default)]
    pub depth_by_type: HashMap<String, u32>,
}

impl Default for LintBlueprintConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            severity: None,
            event_tick_limit: None,
            cast_limit: None,
            node_limit: None,
            dependency_depth_limit: None,
            depth_by_type: HashMap::new(),
        }
    }
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

/// `[check]` — check command settings. `baseline_path`, when set, is the default
/// baseline that `check` auto-diffs against (equivalent to `--diff-from <path>`).
/// Unset means no auto-diff; an explicit `--diff-from` always overrides it.
#[derive(Default, serde::Deserialize)]
pub struct CheckSectionConfig {
    pub baseline_path: Option<PathBuf>,
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

pub fn resolve_config(project_dir: &Path, explicit: Option<&Path>) -> anyhow::Result<ConfigFile> {
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
        std::fs::write(&path, "[lint.blueprint]\ndependency_depth_limit =5\n").unwrap();
        let cfg = load_config_at(&path);
        assert_eq!(cfg.lint.blueprint.dependency_depth_limit, Some(5));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_config_at_should_return_default_when_file_is_missing() {
        let path =
            std::env::temp_dir().join(format!("uasset_lens_absent_{}.toml", std::process::id()));
        // don't create it — we want it to be missing
        let cfg = load_config_at(&path);
        assert!(cfg.lint.blueprint.dependency_depth_limit.is_none());
        assert!(cfg.lint.blueprint.depth_by_type.is_empty());
    }

    #[test]
    fn resolve_config_should_load_from_explicit_path_when_specified() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_res_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("custom.toml");
        std::fs::write(&path, "[lint.blueprint]\ndependency_depth_limit =7\n").unwrap();
        let cfg = resolve_config(std::path::Path::new("."), Some(&path)).unwrap();
        assert_eq!(cfg.lint.blueprint.dependency_depth_limit, Some(7));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_config_should_return_error_when_explicit_path_is_missing() {
        let absent = std::env::temp_dir().join(format!(
            "uasset_lens_absent_err_{}.toml",
            std::process::id()
        ));
        let result = resolve_config(std::path::Path::new("."), Some(&absent));
        assert!(result.is_err());
    }

    #[test]
    fn resolve_config_should_use_project_dir_default_when_no_explicit_path() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_def_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[lint.blueprint]\ndependency_depth_limit =3\n",
        )
        .unwrap();
        let cfg = resolve_config(&dir, None).unwrap();
        assert_eq!(cfg.lint.blueprint.dependency_depth_limit, Some(3));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_config_should_parse_blueprint_max_dependency_depth() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_bdd_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[lint.blueprint]\ndependency_depth_limit =12\n",
        )
        .unwrap();
        let cfg = load_config(&dir);
        assert_eq!(cfg.lint.blueprint.dependency_depth_limit, Some(12));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_config_should_parse_blueprint_depth_by_type() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_dbt_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[lint.blueprint.depth_by_type]\nBlueprint = 10\nAnimBlueprint = 20\n",
        )
        .unwrap();
        let cfg = load_config(&dir);
        assert_eq!(
            cfg.lint.blueprint.depth_by_type.get("Blueprint"),
            Some(&10u32)
        );
        assert_eq!(
            cfg.lint.blueprint.depth_by_type.get("AnimBlueprint"),
            Some(&20u32)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_config_should_parse_rules_severity_from_valid_toml() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_rules_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[rules]\ndead-assets = \"off\"\ncircular-deps = \"error\"\nduplicate-assets = \"warn\"\n",
        )
        .unwrap();
        let cfg = load_config(&dir);
        assert_eq!(cfg.rules.dead_assets, Some(CheckSeverity::Off));
        assert_eq!(cfg.rules.circular_deps, Some(CheckSeverity::Error));
        assert_eq!(cfg.rules.duplicate_assets, Some(CheckSeverity::Warn));
        assert_eq!(cfg.rules.redirectors, None, "absent key stays None");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_config_should_leave_rules_none_when_section_absent() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_norules_{}", std::process::id()));

        let cfg = load_config(&dir);

        assert_eq!(cfg.rules.dead_assets, None);
        assert_eq!(cfg.rules.circular_deps, None);
        assert_eq!(cfg.rules.duplicate_assets, None);
        assert_eq!(cfg.rules.redirectors, None);
    }

    #[test]
    fn load_config_should_parse_nested_lint_naming_fields() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_lnaming_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[lint.naming]\nenabled = false\nseverity = \"warn\"\ntexture_prefix = \"TX_\"\n",
        )
        .unwrap();
        let cfg = load_config(&dir);
        assert!(!cfg.lint.naming.enabled);
        assert_eq!(cfg.lint.naming.severity, Some(CheckSeverity::Warn));
        assert_eq!(cfg.lint.naming.texture_prefix.as_deref(), Some("TX_"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_config_should_parse_nested_lint_blueprint_fields() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_lbp_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[lint.blueprint]\nenabled = false\nseverity = \"error\"\nevent_tick_limit = 2\n",
        )
        .unwrap();
        let cfg = load_config(&dir);
        assert!(!cfg.lint.blueprint.enabled);
        assert_eq!(cfg.lint.blueprint.severity, Some(CheckSeverity::Error));
        assert_eq!(cfg.lint.blueprint.event_tick_limit, Some(2));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_config_should_default_lint_sub_rules_enabled_when_absent() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_ldef_{}", std::process::id()));

        let cfg = load_config(&dir);

        assert!(cfg.lint.naming.enabled, "naming defaults to enabled");
        assert!(cfg.lint.blueprint.enabled, "blueprint defaults to enabled");
        assert_eq!(cfg.lint.naming.severity, None);
    }

    #[test]
    fn load_config_should_parse_check_baseline_path_from_valid_toml() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_check_bp_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[check]\nbaseline_path = \".uasset-lens/baseline.json\"\n",
        )
        .unwrap();
        let cfg = load_config(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(
            cfg.check.baseline_path,
            Some(PathBuf::from(".uasset-lens/baseline.json"))
        );
    }

    #[test]
    fn load_config_should_leave_check_baseline_path_none_when_section_absent() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_check_none_{}", std::process::id()));

        let cfg = load_config(&dir);

        assert_eq!(cfg.check.baseline_path, None);
    }
}
