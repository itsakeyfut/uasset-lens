use std::path::Path;

#[derive(Default, serde::Deserialize)]
pub struct ConfigFile {
    #[serde(default)]
    pub scan: ScanConfig,
}

#[derive(Default, serde::Deserialize)]
pub struct ScanConfig {
    #[serde(default)]
    pub exclude_paths: Vec<String>,
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
}
