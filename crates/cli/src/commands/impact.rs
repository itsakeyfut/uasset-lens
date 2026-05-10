use std::path::{Path, PathBuf};

use anyhow::Context;
use shared::AssetPath;

use crate::FormatKind;

#[derive(serde::Serialize)]
struct ImpactOutput {
    target: String,
    direct: Vec<String>,
    transitive: Vec<String>,
    total: usize,
}

pub fn handle_impact(
    asset_path: &Path,
    db_override: Option<&Path>,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    let (target, db_path) = resolve_target_and_db(asset_path, db_override)?;
    let db = crate::open_db(&db_path)?;
    let graph = crate::load_graph(&db)?;

    if !graph.contains(&target) {
        anyhow::bail!("asset not found in scan data: {}", target.as_str());
    }

    let result = graph.find_impact(&target);
    let total = result.direct.len() + result.transitive.len();

    match format {
        FormatKind::Json => {
            let output = ImpactOutput {
                target: target.as_str().to_owned(),
                direct: result
                    .direct
                    .iter()
                    .map(|p| p.as_str().to_owned()) // clone required: AssetPath is not Copy
                    .collect(),
                transitive: result
                    .transitive
                    .iter()
                    .map(|p| p.as_str().to_owned()) // clone required: AssetPath is not Copy
                    .collect(),
                total,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&output)
                    .context("Failed to serialize impact output to JSON")?
            );
        }
        FormatKind::Text => {
            println!("  Impact Analysis: {}", target.as_str());
            println!();
            println!("  Direct referencing ({}):", result.direct.len());
            for path in &result.direct {
                println!("    {}", path.as_str());
            }
            println!();
            println!("  Transitive referencing ({}):", result.transitive.len());
            let remaining = result.transitive.len().saturating_sub(10);
            for path in result.transitive.iter().take(10) {
                println!("    {}", path.as_str());
            }
            if remaining > 0 {
                println!("    ... ({} more)", remaining);
            }
            println!();
            println!("  Total impact: {} assets", total);
        }
    }

    if total > 0 { Ok(1) } else { Ok(0) }
}

/// Resolves the target `AssetPath` and DB file path from the raw CLI argument.
///
/// Accepts two forms:
/// - Game path (`/Game/...`): parsed directly; DB found by walking up from CWD or via `--db`.
/// - Filesystem path (`.uasset`/`.umap`): converted to game path via `from_fs_path`; project
///   root located by walking up from the file.
fn resolve_target_and_db(
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

    // Filesystem path — walk up from file's parent to locate project root
    let start = asset_path.parent().unwrap_or(asset_path);
    let project_dir = find_project_dir(start)?;
    let content_root = crate::resolve_content_root(&project_dir);
    let target = AssetPath::from_fs_path(&content_root, asset_path)
        .map_err(|e| anyhow::anyhow!("cannot convert path to game path: {e}"))?;
    let db_path = db_override
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| project_dir.join(".uasset-lens").join("uasset-lens.db"));

    Ok((target, db_path))
}

fn find_project_dir(start: &Path) -> anyhow::Result<PathBuf> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::make_meta;
    use shared::{AssetPath, AssetType};

    #[test]
    fn handle_impact_should_return_err_when_db_does_not_exist() {
        let db_path = std::env::temp_dir().join(format!(
            "uasset_lens_impact23_missing_{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);

        let result = handle_impact(Path::new("/Game/Target"), Some(&db_path), &FormatKind::Text);
        assert!(result.is_err(), "missing DB should return an error");
    }

    #[test]
    fn handle_impact_should_return_err_when_target_not_in_graph() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_impact23_notfound_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_impact(
            Path::new("/Game/NotInGraph"),
            Some(&db_path),
            &FormatKind::Text,
        );
        assert!(
            result.is_err(),
            "target not in graph should return an error"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_impact_should_return_0_when_no_referencing_assets() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_impact23_no_refs_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/Target",
                dir.join("Target.uasset"),
                AssetType::Blueprint,
                1024,
                vec![],
            )])
            .unwrap();
        }

        let result =
            handle_impact(Path::new("/Game/Target"), Some(&db_path), &FormatKind::Text).unwrap();
        assert_eq!(result, 0, "no referencing assets means no impact");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_impact_should_return_1_when_direct_references_exist() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_impact23_direct_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            // A→Target: A directly references Target
            db.upsert_all(&[
                make_meta(
                    "/Game/Target",
                    dir.join("Target.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
                make_meta(
                    "/Game/A",
                    dir.join("A.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![AssetPath::new("/Game/Target").unwrap()],
                ),
            ])
            .unwrap();
        }

        let result =
            handle_impact(Path::new("/Game/Target"), Some(&db_path), &FormatKind::Text).unwrap();
        assert_eq!(result, 1, "one direct reference means impact exists");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_impact_should_correctly_separate_direct_and_transitive() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_impact23_transitive_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            // C→B→Target: B is direct, C is transitive
            db.upsert_all(&[
                make_meta(
                    "/Game/Target",
                    dir.join("Target.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
                make_meta(
                    "/Game/B",
                    dir.join("B.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![AssetPath::new("/Game/Target").unwrap()],
                ),
                make_meta(
                    "/Game/C",
                    dir.join("C.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![AssetPath::new("/Game/B").unwrap()],
                ),
            ])
            .unwrap();
        }

        let result =
            handle_impact(Path::new("/Game/Target"), Some(&db_path), &FormatKind::Text).unwrap();
        assert_eq!(result, 1, "both direct and transitive refs make total > 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_impact_json_should_return_0_when_no_impact() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_impact23_json_empty_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/Target",
                dir.join("Target.uasset"),
                AssetType::Blueprint,
                1024,
                vec![],
            )])
            .unwrap();
        }

        let result =
            handle_impact(Path::new("/Game/Target"), Some(&db_path), &FormatKind::Json).unwrap();
        assert_eq!(result, 0, "JSON format exits 0 when no impact");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_impact_json_should_return_1_when_impact_exists() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_impact23_json_found_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/Target",
                    dir.join("Target.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
                make_meta(
                    "/Game/Referencing",
                    dir.join("Referencing.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![AssetPath::new("/Game/Target").unwrap()],
                ),
            ])
            .unwrap();
        }

        let result =
            handle_impact(Path::new("/Game/Target"), Some(&db_path), &FormatKind::Json).unwrap();
        assert_eq!(result, 1, "JSON format exits 1 when impact exists");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_project_dir_should_find_uasset_lens_dir_in_ancestor() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_impact23_projdir_{}",
            std::process::id()
        ));
        let sub = dir.join("Content").join("Characters");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::create_dir_all(dir.join(".uasset-lens")).unwrap();

        let result = find_project_dir(&sub).unwrap();
        assert_eq!(result, dir);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_project_dir_should_return_err_when_no_uasset_lens_dir_found() {
        // Nonexistent path under temp_dir — the system temp directory and its ancestors
        // are guaranteed to have no .uasset-lens (that dir only lives under the project root)
        let nonexistent = std::env::temp_dir().join(format!(
            "uasset_lens_impact23_projdir_miss_{}",
            std::process::id()
        ));
        assert!(
            find_project_dir(&nonexistent).is_err(),
            "no .uasset-lens in path should return error"
        );
    }
}
