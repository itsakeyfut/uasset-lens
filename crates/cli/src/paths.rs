use std::path::{Path, PathBuf};

use anyhow::Context;
use shared::AssetPath;

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

pub(crate) fn resolve_target_and_db(
    asset_path: &Path,
    db_override: Option<&Path>,
) -> anyhow::Result<(AssetPath, PathBuf)> {
    if asset_path.starts_with("/Game") {
        let s = asset_path.to_string_lossy();
        let target = AssetPath::new(&s).map_err(|e| anyhow::anyhow!("invalid game path: {e}"))?;
        let db_path = match db_override {
            Some(p) => p.to_path_buf(),
            None => {
                let cwd = std::env::current_dir().context("Failed to get current directory")?;
                let project_dir = find_project_dir(&cwd)?;
                project_dir.join(".uasset-lens").join("uasset-lens.db")
            }
        };
        return Ok((target, db_path));
    }

    let start = asset_path.parent().unwrap_or(asset_path);
    let project_dir = find_project_dir(start)?;
    let content_root = resolve_content_root(&project_dir);
    let target = AssetPath::from_fs_path(&content_root, asset_path)
        .map_err(|e| anyhow::anyhow!("cannot convert path to game path: {e}"))?;
    let db_path = db_override
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| project_dir.join(".uasset-lens").join("uasset-lens.db"));

    Ok((target, db_path))
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
    fn resolve_target_and_db_should_return_game_asset_path_and_override_db_when_given_game_path() {
        let db_path =
            std::env::temp_dir().join(format!("uasset_lens_rtdb_game_{}.db", std::process::id()));
        let (target, resolved_db) =
            resolve_target_and_db(Path::new("/Game/Characters/BP_Hero"), Some(&db_path)).unwrap();
        assert_eq!(target.as_str(), "/Game/Characters/BP_Hero");
        assert_eq!(resolved_db, db_path);
    }

    #[test]
    fn resolve_target_and_db_should_convert_fs_path_to_game_path_using_content_root() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_rtdb_fs_{}", std::process::id()));
        let chars = dir.join("Content").join("Characters");
        std::fs::create_dir_all(dir.join(".uasset-lens")).unwrap();
        std::fs::create_dir_all(&chars).unwrap();
        let asset_file = chars.join("BP_Hero.uasset");
        std::fs::write(&asset_file, b"").unwrap();
        let db_path = dir.join(".uasset-lens").join("uasset-lens.db");

        let result = resolve_target_and_db(&asset_file, Some(&db_path));
        let _ = std::fs::remove_dir_all(&dir);

        let (target, resolved_db) = result.unwrap();
        assert_eq!(target.as_str(), "/Game/Characters/BP_Hero");
        assert_eq!(resolved_db, db_path);
    }

    #[test]
    fn resolve_target_and_db_should_return_err_when_fs_path_has_no_project_root() {
        let dir = std::env::temp_dir()
            .join(format!("uasset_lens_rtdb_no_root_{}", std::process::id()))
            .join("Content");
        std::fs::create_dir_all(&dir).unwrap();
        let asset_file = dir.join("BP_Orphan.uasset");
        std::fs::write(&asset_file, b"").unwrap();

        let db_path = std::env::temp_dir().join("dummy.db");
        let result = resolve_target_and_db(&asset_file, Some(&db_path));
        let _ = std::fs::remove_dir_all(
            std::env::temp_dir().join(format!("uasset_lens_rtdb_no_root_{}", std::process::id())),
        );
        assert!(
            result.is_err(),
            "missing project root should return an error"
        );
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
}
