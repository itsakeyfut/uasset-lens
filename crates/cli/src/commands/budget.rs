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
) -> anyhow::Result<i32> {
    crate::maybe_hint_github_actions(format);
    let db = crate::open_db(db_path)?;

    let assets = db
        .all_assets()
        .context("Failed to read assets from database")?;

    let report = budget_tracker::check_budget(
        &assets,
        &budget_tracker::BudgetConfig::effective(&cfg.budget),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_db_in_tempdir;
    use shared::AssetType;

    #[test]
    fn handle_budget_github_actions_should_return_0_when_no_violations() {
        let (dir, db_path) = test_db_in_tempdir("budget_ga_ok");
        std::fs::write(
            dir.join(".uasset-lens.toml"),
            "[budget]\nTexture2D.max_size = 10000\n",
        )
        .unwrap();

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
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
        let result = handle_budget(&dir, &db_path, &cfg, &FormatKind::GithubActions).unwrap();

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
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
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
        let result = handle_budget(&dir, &db_path, &cfg, &FormatKind::GithubActions).unwrap();

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
        );
        assert!(result.is_err());
    }

    #[test]
    fn handle_budget_should_return_0_when_no_assets_in_db() {
        let (dir, db_path) = test_db_in_tempdir("budget_empty");
        asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_budget(&dir, &db_path, &Default::default(), &FormatKind::Text).unwrap();

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
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
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
        let result = handle_budget(&dir, &db_path, &cfg, &FormatKind::Text).unwrap();

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
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
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
        let result = handle_budget(&dir, &db_path, &cfg, &FormatKind::Text).unwrap();

        assert_eq!(result, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_budget_should_return_1_when_texture_exceeds_4mb_built_in_default() {
        let (dir, db_path) = test_db_in_tempdir("budget_builtin_dflt");
        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[crate::commands::make_meta(
                "/Game/T_Large",
                dir.join("T_Large.uasset"),
                AssetType::Texture2D,
                5 * 1024 * 1024,
                vec![],
            )])
            .unwrap();
        }
        let result = handle_budget(&dir, &db_path, &Default::default(), &FormatKind::Text).unwrap();
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
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
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
        let result = handle_budget(&dir, &db_path, &cfg, &FormatKind::Json).unwrap();

        assert_eq!(result, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
