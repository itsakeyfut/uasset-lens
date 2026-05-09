use std::path::{Path, PathBuf};

pub fn resolve_db_path(project_dir: &Path, db_override: Option<&Path>) -> PathBuf {
    db_override
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| project_dir.join(".uasset-lens").join("uasset-lens.db"))
}

pub fn resolve_content_root(project_dir: &Path) -> PathBuf {
    let content = project_dir.join("Content");
    if content.is_dir() {
        content
    } else {
        project_dir.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn resolve_db_path_should_use_override_when_provided() {
        let project = Path::new("proj");
        let override_path = Path::new("custom/my.db");
        assert_eq!(
            resolve_db_path(project, Some(override_path)),
            PathBuf::from("custom/my.db")
        );
    }

    #[test]
    fn resolve_db_path_should_use_default_location_when_no_override() {
        let project = Path::new("proj");
        let expected = PathBuf::from("proj")
            .join(".uasset-lens")
            .join("uasset-lens.db");
        assert_eq!(resolve_db_path(project, None), expected);
    }

    #[test]
    fn resolve_content_root_should_use_content_subdir_when_it_exists() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_cli_content_{}", std::process::id()));
        let content = dir.join("Content");
        std::fs::create_dir_all(&content).unwrap();
        let result = resolve_content_root(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(result, content);
    }

    #[test]
    fn resolve_content_root_should_use_project_dir_when_no_content_subdir() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_cli_no_content_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let result = resolve_content_root(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(result, dir);
    }
}
