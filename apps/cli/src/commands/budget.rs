use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::Context;

use crate::FormatKind;

#[derive(serde::Serialize)]
struct BudgetOutput {
    violations: Vec<BudgetViolationEntry>,
    total: usize,
}

#[derive(serde::Serialize)]
struct BudgetViolationEntry {
    asset_path: String,
    asset_type: String,
    file_size: u64,
    max_size: u64,
}

pub fn handle_budget(
    project_dir: &Path,
    db_path: &Path,
    cfg: &crate::config::ConfigFile,
    format: &FormatKind,
    quiet: bool,
) -> anyhow::Result<i32> {
    crate::maybe_hint_github_actions(format, quiet);
    let db = crate::open_db(db_path)?;

    let assets = db
        .all_assets()
        .context("Failed to read assets from database")?;

    let report = uasset_lens_analysis::check_budget(
        &assets,
        &uasset_lens_analysis::BudgetConfig::effective(&cfg.budget),
    );

    let entries: Vec<BudgetViolationEntry> = report
        .violations
        .iter()
        .map(|v| BudgetViolationEntry {
            asset_path: v.asset_path.as_str().to_owned(),
            asset_type: v.asset_type.to_string(),
            file_size: v.file_size,
            max_size: v.max_size,
        })
        .collect();

    let has_violations = !entries.is_empty();

    match format {
        FormatKind::Sarif => {
            let findings = build_budget_findings(&report.violations, &assets, project_dir);
            println!("{}", crate::sarif::to_sarif_json(&findings)?);
        }
        FormatKind::GithubActions => {
            let path_lookup: HashMap<&str, &std::path::Path> = assets
                .iter()
                .map(|r| (r.asset_path.as_str(), r.file_path.as_path()))
                .collect();
            for e in &entries {
                let rel = crate::rel_path_for_annotation(
                    path_lookup
                        .get(e.asset_path.as_str())
                        .copied()
                        .unwrap_or(std::path::Path::new("")),
                    project_dir,
                );
                let msg = format!(
                    "{} {} exceeds limit {}",
                    e.asset_type,
                    crate::format_size(e.file_size),
                    crate::format_size(e.max_size)
                );
                println!(
                    "{}",
                    crate::format_gh_annotation("error", &rel, "BudgetOverrun", &msg)
                );
            }
            return if has_violations { Ok(1) } else { Ok(0) };
        }
        FormatKind::Json => {
            let total = entries.len();
            let output = BudgetOutput {
                violations: entries,
                total,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&output)
                    .context("Failed to serialize budget output to JSON")?
            );
        }
        FormatKind::Text => {
            println!("Budget Report");
            println!("=============");
            if entries.is_empty() {
                println!("  (no budget violations found)");
            } else {
                let mut groups: BTreeMap<&str, Vec<&BudgetViolationEntry>> = BTreeMap::new();
                for entry in &entries {
                    groups
                        .entry(entry.asset_type.as_str())
                        .or_default()
                        .push(entry);
                }
                for (type_name, group) in &groups {
                    let max_size = group[0].max_size;
                    println!("{} (limit: {})", type_name, crate::format_size(max_size));
                    for e in group {
                        let excess = e.file_size - e.max_size;
                        println!(
                            "  {:<50}  {:>8}  [+{}]",
                            e.asset_path,
                            crate::format_size(e.file_size),
                            crate::format_size(excess),
                        );
                    }
                    println!();
                }
                let total = entries.len();
                let type_count = groups.len();
                println!(
                    "Summary: {} {} across {} asset {}.",
                    total,
                    if total == 1 {
                        "violation"
                    } else {
                        "violations"
                    },
                    type_count,
                    if type_count == 1 { "type" } else { "types" },
                );
            }
        }
    }

    if has_violations { Ok(1) } else { Ok(0) }
}

/// Converts budget violations into SARIF findings (always error level), resolving each
/// `asset_path` to a project-relative file uri via the asset table.
fn build_budget_findings(
    violations: &[uasset_lens_analysis::budget::BudgetViolation],
    assets: &[uasset_lens_asset_db::AssetRecord],
    project_dir: &std::path::Path,
) -> Vec<crate::sarif::SarifFinding> {
    let path_lookup: HashMap<&str, &std::path::Path> = assets
        .iter()
        .map(|r| (r.asset_path.as_str(), r.file_path.as_path()))
        .collect();
    violations
        .iter()
        .map(|v| crate::sarif::SarifFinding {
            rule_id: crate::budget_rule_id(&v.asset_type),
            level: crate::sarif::SarifLevel::Error,
            message: format!(
                "{} {} exceeds limit {}",
                v.asset_type,
                crate::format_size(v.file_size),
                crate::format_size(v.max_size),
            ),
            uri: path_lookup
                .get(v.asset_path.as_str())
                .map(|p| crate::rel_path_for_annotation(p, project_dir)),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_db_in_tempdir;
    use uasset_lens_shared::AssetType;

    #[test]
    fn handle_budget_github_actions_should_return_0_when_no_violations() {
        let (dir, db_path) = test_db_in_tempdir("budget_ga_ok");
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[budget]\nTexture2D.max_size = 10000\n",
        )
        .unwrap();

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[crate::commands::make_meta(
                "/Game/T_Small",
                dir.join("T_Small.uasset"),
                AssetType::Texture2D,
                1000,
                vec![],
            )])
            .unwrap();
        }

        let cfg = crate::config::load_config(&dir);
        let result =
            handle_budget(&dir, &db_path, &cfg, &FormatKind::GithubActions, false).unwrap();

        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_budget_github_actions_should_return_1_when_violations_found() {
        let (dir, db_path) = test_db_in_tempdir("budget_ga_over");
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[budget]\nTexture2D.max_size = 1\n",
        )
        .unwrap();

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[crate::commands::make_meta(
                "/Game/T_Large",
                dir.join("T_Large.uasset"),
                AssetType::Texture2D,
                1024,
                vec![],
            )])
            .unwrap();
        }

        let cfg = crate::config::load_config(&dir);
        let result =
            handle_budget(&dir, &db_path, &cfg, &FormatKind::GithubActions, false).unwrap();

        assert_eq!(result, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_budget_should_return_err_when_db_does_not_exist() {
        let (dir, db_path) = test_db_in_tempdir("budget_missing");
        let _ = std::fs::remove_dir_all(&dir);
        let result = handle_budget(
            std::path::Path::new("."),
            &db_path,
            &Default::default(),
            &FormatKind::Text,
            false,
        );
        assert!(result.is_err());
    }

    #[test]
    fn handle_budget_should_return_0_when_no_assets_in_db() {
        let (dir, db_path) = test_db_in_tempdir("budget_empty");
        uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_budget(
            &dir,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
            false,
        )
        .unwrap();

        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_budget_should_return_0_when_all_assets_within_budget() {
        let (dir, db_path) = test_db_in_tempdir("budget_ok");
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[budget]\nTexture2D.max_size = 10000\n",
        )
        .unwrap();

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[crate::commands::make_meta(
                "/Game/T_Small",
                dir.join("T_Small.uasset"),
                AssetType::Texture2D,
                1000,
                vec![],
            )])
            .unwrap();
        }

        let cfg = crate::config::load_config(&dir);
        let result = handle_budget(&dir, &db_path, &cfg, &FormatKind::Text, false).unwrap();

        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_budget_should_return_1_when_asset_exceeds_budget() {
        let (dir, db_path) = test_db_in_tempdir("budget_over");
        // max_size = 1 byte so any real asset triggers a violation
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[budget]\nTexture2D.max_size = 1\n",
        )
        .unwrap();

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[crate::commands::make_meta(
                "/Game/T_Large",
                dir.join("T_Large.uasset"),
                AssetType::Texture2D,
                1024,
                vec![],
            )])
            .unwrap();
        }

        let cfg = crate::config::load_config(&dir);
        let result = handle_budget(&dir, &db_path, &cfg, &FormatKind::Text, false).unwrap();

        assert_eq!(result, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_budget_should_return_1_when_texture_exceeds_4mb_built_in_default() {
        let (dir, db_path) = test_db_in_tempdir("budget_builtin_dflt");
        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[crate::commands::make_meta(
                "/Game/T_Large",
                dir.join("T_Large.uasset"),
                AssetType::Texture2D,
                5 * 1024 * 1024,
                vec![],
            )])
            .unwrap();
        }
        let result = handle_budget(
            &dir,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
            false,
        )
        .unwrap();
        assert_eq!(result, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_budget_json_should_return_1_when_violations_found() {
        let (dir, db_path) = test_db_in_tempdir("budget_json");
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[budget]\nTexture2D.max_size = 1\n",
        )
        .unwrap();

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[crate::commands::make_meta(
                "/Game/T_Large",
                dir.join("T_Large.uasset"),
                AssetType::Texture2D,
                1024,
                vec![],
            )])
            .unwrap();
        }

        let cfg = crate::config::load_config(&dir);
        let result = handle_budget(&dir, &db_path, &cfg, &FormatKind::Json, false).unwrap();

        assert_eq!(result, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_budget_sarif_should_return_1_when_violation_found() {
        let (dir, db_path) = test_db_in_tempdir("budget_sarif");
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[budget]\nTexture2D.max_size = 1\n",
        )
        .unwrap();
        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[crate::commands::make_meta(
                "/Game/T_Large",
                dir.join("T_Large.uasset"),
                AssetType::Texture2D,
                1024,
                vec![],
            )])
            .unwrap();
        }
        let cfg = crate::config::load_config(&dir);
        let result = handle_budget(&dir, &db_path, &cfg, &FormatKind::Sarif, false).unwrap();
        assert_eq!(result, 1, "sarif mode keeps the format-agnostic exit code");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_budget_findings_should_use_error_level_and_canonical_rule_id() {
        use std::path::PathBuf;
        use uasset_lens_shared::AssetPath;
        let assets = vec![uasset_lens_asset_db::AssetRecord {
            id: 0,
            asset_path: AssetPath::new("/Game/T_Big").unwrap(),
            file_path: PathBuf::from("/proj/Content/T_Big.uasset"),
            asset_type: AssetType::Texture2D,
            file_size: 5_000_000,
            last_modified: 0,
        }];
        let violations = vec![uasset_lens_analysis::budget::BudgetViolation {
            asset_path: AssetPath::new("/Game/T_Big").unwrap(),
            asset_type: AssetType::Texture2D,
            file_size: 5_000_000,
            max_size: 4_000_000,
        }];
        let f = build_budget_findings(&violations, &assets, std::path::Path::new("/proj"));
        assert_eq!(f[0].rule_id, "budget/texture2d");
        assert!(matches!(f[0].level, crate::sarif::SarifLevel::Error));
        assert_eq!(f[0].uri.as_deref(), Some("Content/T_Big.uasset"));
    }
}
