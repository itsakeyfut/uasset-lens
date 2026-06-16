use std::path::{Path, PathBuf};

use uasset_lens_shared::AssetPath;

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

pub(crate) fn find_project_dir(start: &Path) -> anyhow::Result<PathBuf> {
    let mut dir = start;
    loop {
        if dir.join(".uasset-lens").is_dir() {
            return Ok(dir.to_path_buf());
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => {
                anyhow::bail!("no scan data found.\nRun 'uasset-lens scan <project_dir>' first.")
            }
        }
    }
}

pub(crate) fn resolve_asset_path(
    project_dir: &Path,
    asset_path: &Path,
) -> anyhow::Result<AssetPath> {
    if asset_path.to_str().is_some_and(|s| s.starts_with("/Game")) {
        let s = asset_path.to_string_lossy();
        AssetPath::new(&s).map_err(|e| anyhow::anyhow!("invalid game path: {e}"))
    } else {
        let content_root = resolve_content_root(project_dir);
        AssetPath::from_fs_path(&content_root, asset_path)
            .map_err(|e| anyhow::anyhow!("cannot convert path to game path: {e}"))
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

    #[test]
    fn find_project_dir_should_return_dir_when_marker_present() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_find_proj_{}", std::process::id()));
        let marker = dir.join(".uasset-lens");
        std::fs::create_dir_all(&marker).unwrap();
        let result = find_project_dir(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(result, dir);
    }

    #[test]
    fn find_project_dir_should_walk_up_to_find_marker() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_find_proj_walk_{}", std::process::id()));
        let marker = dir.join(".uasset-lens");
        let nested = dir.join("Content").join("Characters");
        std::fs::create_dir_all(&marker).unwrap();
        std::fs::create_dir_all(&nested).unwrap();
        let result = find_project_dir(&nested).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(result, dir);
    }

    #[test]
    fn find_project_dir_should_return_err_when_no_marker_found() {
        // Create a leaf directory with no .uasset-lens anywhere in the temp tree.
        // The temp root (e.g. C:\Users\...\AppData\Local\Temp) has no .uasset-lens,
        // so walking up from our leaf will reach the filesystem root and bail.
        let dir = std::env::temp_dir()
            .join(format!("uasset_lens_find_proj_none_{}", std::process::id()))
            .join("deep")
            .join("leaf");
        std::fs::create_dir_all(&dir).unwrap();
        let result = find_project_dir(&dir);
        let _ = std::fs::remove_dir_all(
            std::env::temp_dir().join(format!("uasset_lens_find_proj_none_{}", std::process::id())),
        );
        assert!(
            result.is_err(),
            "should fail when no .uasset-lens marker exists"
        );
    }

    #[test]
    fn resolve_asset_path_should_parse_game_path_directly() {
        let dir = Path::new("/some/project");
        let asset = Path::new("/Game/Characters/BP_Player");
        let result = resolve_asset_path(dir, asset).unwrap();
        assert_eq!(result.as_str(), "/Game/Characters/BP_Player");
    }

    #[test]
    fn resolve_asset_path_should_convert_fs_path_via_content_root() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_resolve_path_{}", std::process::id()));
        let content = dir.join("Content");
        let chars = content.join("Characters");
        std::fs::create_dir_all(&chars).unwrap();
        let asset = chars.join("BP_Player.uasset");
        std::fs::write(&asset, b"").unwrap();
        let result = resolve_asset_path(&dir, &asset).unwrap();
        assert_eq!(result.as_str(), "/Game/Characters/BP_Player");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_asset_path_should_return_err_for_fs_path_outside_content_root() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_resolve_path_err_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        // No Content subdirectory — an asset at /tmp/outside.uasset cannot be resolved
        let asset =
            std::env::temp_dir().join(format!("uasset_lens_outside_{}.uasset", std::process::id()));
        let result = resolve_asset_path(&dir, &asset);
        assert!(result.is_err(), "path outside content root should fail");
        let _ = std::fs::remove_file(&asset);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
