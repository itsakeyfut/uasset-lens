use std::path::Path;

use anyhow::Context;

use crate::FormatKind;

#[derive(serde::Serialize)]
struct RedirectorsOutput {
    count: usize,
    redirectors: Vec<String>,
}

pub fn handle_redirectors(
    _project_dir: &Path,
    db_path: &Path,
    cfg: &crate::config::ConfigFile,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    let db = crate::open_db(db_path)?;
    let graph = crate::load_graph(&db, &cfg.scan.external_roots)?;
    let paths = redirector_analyzer::detect(&graph);

    let redirector_paths: Vec<String> = paths
        .iter()
        .map(|p| p.as_str().to_owned()) // clone required: AssetPath is not Copy
        .collect();
    let found = !redirector_paths.is_empty();

    match format {
        FormatKind::Json => {
            let count = redirector_paths.len();
            let output = RedirectorsOutput {
                count,
                redirectors: redirector_paths,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&output)
                    .context("Failed to serialize redirectors output to JSON")?
            );
        }
        FormatKind::GithubActions | FormatKind::Text => {
            let header = format!("ObjectRedirectors ({} found)", redirector_paths.len());
            let separator = "=".repeat(header.len());
            println!("{header}");
            println!("{separator}");
            for path in &redirector_paths {
                println!("{path}");
            }
            println!();
            println!("Note: redirect target resolution is available in Phase 4 analysis.");
        }
    }

    if found { Ok(1) } else { Ok(0) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{make_meta, test_db_in_tempdir};
    use shared::AssetType;

    #[test]
    fn handle_redirectors_should_return_err_when_db_does_not_exist() {
        let (dir, db_path) = test_db_in_tempdir("redir26_missing");
        let _ = std::fs::remove_dir_all(&dir);

        let result = handle_redirectors(
            Path::new("/proj"),
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        );
        assert!(result.is_err(), "missing DB should return an error");
    }

    #[test]
    fn handle_redirectors_should_return_0_when_no_redirectors_found() {
        let (dir, db_path) = test_db_in_tempdir("redir26_empty");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/BP_Player",
                dir.join("BP_Player.uasset"),
                AssetType::Blueprint,
                1024,
                vec![],
            )])
            .unwrap();
        }

        let result =
            handle_redirectors(&dir, &db_path, &Default::default(), &FormatKind::Text).unwrap();
        assert_eq!(result, 0, "no redirectors → exit 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_redirectors_should_return_1_when_redirectors_found() {
        let (dir, db_path) = test_db_in_tempdir("redir26_found");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/OldName",
                dir.join("OldName.uasset"),
                AssetType::ObjectRedirector,
                1024,
                vec![],
            )])
            .unwrap();
        }

        let result =
            handle_redirectors(&dir, &db_path, &Default::default(), &FormatKind::Text).unwrap();
        assert_eq!(result, 1, "redirector found → exit 1");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_redirectors_json_should_return_0_when_no_redirectors_found() {
        let (dir, db_path) = test_db_in_tempdir("redir26_json_empty");
        asset_db::AssetDb::open(&db_path).unwrap();

        let result =
            handle_redirectors(&dir, &db_path, &Default::default(), &FormatKind::Json).unwrap();
        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_redirectors_json_should_return_1_when_redirectors_found() {
        let (dir, db_path) = test_db_in_tempdir("redir26_json_found");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/OldMesh",
                    dir.join("OldMesh.uasset"),
                    AssetType::ObjectRedirector,
                    1024,
                    vec![],
                ),
                make_meta(
                    "/Game/OldBP",
                    dir.join("OldBP.uasset"),
                    AssetType::ObjectRedirector,
                    1024,
                    vec![],
                ),
            ])
            .unwrap();
        }

        let result =
            handle_redirectors(&dir, &db_path, &Default::default(), &FormatKind::Json).unwrap();
        assert_eq!(result, 1, "JSON format exits 1 when redirectors are found");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
