use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;

use crate::FormatKind;

#[derive(serde::Serialize)]
struct LintEntry {
    severity: String,
    rule_id: String,
    asset_path: String,
    message: String,
}

pub fn handle_lint(project_dir: &Path, db_path: &Path, format: &FormatKind) -> anyhow::Result<i32> {
    crate::maybe_hint_github_actions(format);
    let config = crate::config::load_config(project_dir);
    let db = crate::open_db(db_path)?;

    let engine = lint_engine::LintEngine::new(crate::lint_builder::build_lint_rules(&config.lint));

    let assets = db
        .all_assets()
        .context("Failed to read assets from database")?;

    let bp_rows = db
        .all_blueprint_metrics()
        .context("Failed to read blueprint metrics from database")?;

    let metrics_map: HashMap<shared::AssetPath, scanner::BlueprintMetrics> = bp_rows
        .into_iter()
        .map(|row| {
            (
                row.asset_path,
                scanner::BlueprintMetrics {
                    node_count: row.node_count,
                    event_tick_count: row.event_tick_count,
                    cast_count: row.cast_count,
                    dependency_depth: row.dependency_depth,
                },
            )
        })
        .collect();

    let violations = engine.run(&assets, &metrics_map);

    let entries: Vec<LintEntry> = violations
        .iter()
        .map(|v| LintEntry {
            severity: match v.severity {
                lint_engine::Severity::Error => "error".to_owned(),
                lint_engine::Severity::Warning => "warning".to_owned(),
            },
            rule_id: v.rule_id.to_owned(),
            asset_path: v.asset_path.as_str().to_owned(),
            message: v.message.clone(), // clone required: LintEntry owns the message
        })
        .collect();

    match format {
        FormatKind::GithubActions => {
            let path_lookup: HashMap<&str, &std::path::Path> = assets
                .iter()
                .map(|r| (r.asset_path.as_str(), r.file_path.as_path()))
                .collect();
            let has_error = violations
                .iter()
                .any(|v| v.severity == lint_engine::Severity::Error);
            for v in &violations {
                let rel = crate::rel_path_for_annotation(
                    path_lookup
                        .get(v.asset_path.as_str())
                        .copied()
                        .unwrap_or(std::path::Path::new("")),
                    project_dir,
                );
                let level = match v.severity {
                    lint_engine::Severity::Error => "error",
                    lint_engine::Severity::Warning => "warning",
                };
                println!(
                    "{}",
                    crate::format_gh_annotation(level, &rel, v.rule_id, &v.message)
                );
            }
            return if has_error { Ok(1) } else { Ok(0) };
        }
        FormatKind::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&entries)
                    .context("Failed to serialize lint output to JSON")?
            );
        }
        FormatKind::Text => {
            println!("Lint Results");
            println!("============");
            if entries.is_empty() {
                println!("  (no violations found)");
            } else {
                for e in &entries {
                    println!(
                        "{:<8}  {:<26}  {:<40}  {}",
                        e.severity.to_uppercase(),
                        e.rule_id,
                        truncate_path(&e.asset_path, 40),
                        e.message,
                    );
                }
                println!();
                let n = entries.len();
                println!(
                    "{} {} found.",
                    n,
                    if n == 1 { "violation" } else { "violations" }
                );
            }
        }
    }

    if entries.is_empty() { Ok(0) } else { Ok(1) }
}

fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        path.to_owned()
    } else {
        // UE5 asset paths are ASCII; byte boundary aligns with char boundary.
        let start = path.len().saturating_sub(max_len - 3);
        format!("...{}", &path[start..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::{AssetPath, AssetType};
    use std::path::PathBuf;

    fn make_bp_asset(path: &str, dir: &std::path::Path) -> scanner::AssetMetadata {
        scanner::AssetMetadata {
            asset_path: AssetPath::new(path).unwrap(),
            file_path: dir.join(PathBuf::from(path.trim_start_matches('/'))),
            asset_type: AssetType::Blueprint,
            file_size: 1024,
            last_modified: 0,
            dependencies: vec![],
            soft_dependencies: vec![],
            blueprint_metrics: None,
            material_texture_samples: None,
        }
    }

    #[test]
    fn handle_lint_github_actions_should_return_0_when_no_violations() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_lint_ga_empty_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_lint(&dir, &db_path, &FormatKind::GithubActions).unwrap();

        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_lint_github_actions_should_return_0_when_only_warnings() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_lint_ga_warn_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_bp_asset("/Game/Blueprints/Rock_BP", &dir)])
                .unwrap();
        }

        let result = handle_lint(&dir, &db_path, &FormatKind::GithubActions).unwrap();

        assert_eq!(
            result, 0,
            "warnings-only should exit 0 in github-actions mode"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_lint_github_actions_should_return_1_when_error_violation_found() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_lint_ga_err_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[scanner::AssetMetadata {
                asset_path: AssetPath::new("/Game/BP_Boss").unwrap(),
                file_path: dir.join("BP_Boss.uasset"),
                asset_type: AssetType::Blueprint,
                file_size: 4096,
                last_modified: 0,
                dependencies: vec![],
                soft_dependencies: vec![],
                blueprint_metrics: Some(scanner::BlueprintMetrics {
                    node_count: 201,
                    event_tick_count: 0,
                    cast_count: 0,
                    dependency_depth: 0,
                }),
                material_texture_samples: None,
            }])
            .unwrap();
        }

        let result = handle_lint(&dir, &db_path, &FormatKind::GithubActions).unwrap();

        assert_eq!(
            result, 1,
            "error-level violation should exit 1 in github-actions mode"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_lint_should_return_err_when_db_does_not_exist() {
        let path = std::env::temp_dir().join(format!(
            "uasset_lens_lint_missing_{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let result = handle_lint(std::path::Path::new("."), &path, &FormatKind::Text);
        assert!(result.is_err());
    }

    #[test]
    fn handle_lint_should_return_0_when_no_assets_in_db() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_lint_empty_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_lint(&dir, &db_path, &FormatKind::Text).unwrap();

        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_lint_should_return_1_when_naming_violation_found() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_lint_naming_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            // "Rock_BP" does not start with "BP_" — naming/prefix violation
            db.upsert_all(&[make_bp_asset("/Game/Blueprints/Rock_BP", &dir)])
                .unwrap();
        }

        let result = handle_lint(&dir, &db_path, &FormatKind::Text).unwrap();

        assert_eq!(result, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_lint_should_return_0_when_all_assets_are_compliant() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_lint_ok_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_bp_asset("/Game/Blueprints/BP_Player", &dir)])
                .unwrap();
        }

        let result = handle_lint(&dir, &db_path, &FormatKind::Text).unwrap();

        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_lint_should_return_1_with_violations_in_json_mode() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_lint_json_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_bp_asset("/Game/Blueprints/Rock_BP", &dir)])
                .unwrap();
        }

        let result = handle_lint(&dir, &db_path, &FormatKind::Json).unwrap();

        assert_eq!(result, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_lint_should_return_1_when_blueprint_complexity_violation_found() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_lint_bp_cmplx_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            // node_count 201 exceeds the default BlueprintComplexityRule threshold of 200
            db.upsert_all(&[scanner::AssetMetadata {
                asset_path: AssetPath::new("/Game/BP_Boss").unwrap(),
                file_path: dir.join("BP_Boss.uasset"),
                asset_type: AssetType::Blueprint,
                file_size: 4096,
                last_modified: 0,
                dependencies: vec![],
                soft_dependencies: vec![],
                blueprint_metrics: Some(scanner::BlueprintMetrics {
                    node_count: 201,
                    event_tick_count: 0,
                    cast_count: 0,
                    dependency_depth: 0,
                }),
                material_texture_samples: None,
            }])
            .unwrap();
        }

        let result = handle_lint(&dir, &db_path, &FormatKind::Text).unwrap();

        assert_eq!(result, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
