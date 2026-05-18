use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Context;

use crate::FormatKind;

const ALL_CHECKS: &[&str] = &[
    "dead-assets",
    "cycles",
    "redirectors",
    "lint",
    "budget",
    "duplicates",
];

struct CheckResult {
    name: &'static str,
    passed: bool,
    summary: String,
    findings: Vec<String>,
}

#[derive(serde::Serialize)]
struct CheckResultJson {
    name: String,
    passed: bool,
    findings: Vec<String>,
}

#[derive(serde::Serialize)]
struct CheckOutput {
    passed: bool,
    checks: Vec<CheckResultJson>,
}

pub fn handle_check(
    project_dir: &Path,
    only: &[String],
    skip: &[String],
    db_path: &Path,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    for name in only.iter().chain(skip.iter()) {
        if !ALL_CHECKS.contains(&name.as_str()) {
            anyhow::bail!(
                "unknown check '{}'. Valid names: {}",
                name,
                ALL_CHECKS.join(", ")
            );
        }
    }

    let active: Vec<&'static str> = ALL_CHECKS
        .iter()
        .copied()
        .filter(|&n| {
            (only.is_empty() || only.iter().any(|o| o == n)) && !skip.iter().any(|s| s == n)
        })
        .collect();

    let db = crate::open_db(db_path)?;
    let config = crate::config::load_config(project_dir);

    let needs_graph = active
        .iter()
        .any(|&n| matches!(n, "dead-assets" | "cycles" | "redirectors"));
    let needs_assets = active
        .iter()
        .any(|&n| matches!(n, "lint" | "budget" | "duplicates"));

    let graph = if needs_graph {
        Some(crate::load_graph(&db)?)
    } else {
        None
    };

    let assets = if needs_assets {
        Some(
            db.all_assets()
                .context("Failed to read assets from database")?,
        )
    } else {
        None
    };

    let (bp_metrics_map, lint_engine) = if active.contains(&"lint") {
        let rows = db
            .all_blueprint_metrics()
            .context("Failed to read blueprint metrics from database")?;
        let map: HashMap<shared::AssetPath, scanner::BlueprintMetrics> = rows
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
        let engine =
            lint_engine::LintEngine::new(crate::lint_builder::build_lint_rules(&config.lint));
        (Some(map), Some(engine))
    } else {
        (None, None)
    };

    let mut results: Vec<CheckResult> = Vec::new();

    for &name in &active {
        let result = match name {
            "dead-assets" => {
                let g = graph.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("internal: graph not loaded for dead-assets check")
                })?;
                let dead = dead_asset_detector::detect(g);
                let count = dead.len();
                let findings = dead.iter().map(|p| p.as_str().to_owned()).collect();
                CheckResult {
                    name,
                    passed: count == 0,
                    summary: format!("{count} dead assets"),
                    findings,
                }
            }
            "cycles" => {
                let g = graph.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("internal: graph not loaded for cycles check")
                })?;
                let cycles = g.find_cycles();
                let count = cycles.len();
                let findings = cycles
                    .iter()
                    .map(|scc| {
                        let mut parts: Vec<String> =
                            scc.iter().map(|p| p.as_str().to_owned()).collect();
                        if let Some(first) = parts.first().cloned() {
                            parts.push(first);
                        }
                        parts.join(" \u{2192} ")
                    })
                    .collect();
                CheckResult {
                    name,
                    passed: count == 0,
                    summary: format!("{count} cycles"),
                    findings,
                }
            }
            "redirectors" => {
                let g = graph.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("internal: graph not loaded for redirectors check")
                })?;
                let paths = redirector_analyzer::detect(g);
                let count = paths.len();
                let findings = paths.iter().map(|p| p.as_str().to_owned()).collect();
                CheckResult {
                    name,
                    passed: count == 0,
                    summary: format!("{count} redirectors"),
                    findings,
                }
            }
            "lint" => {
                let engine = lint_engine
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("internal: lint engine not initialized"))?;
                let metrics = bp_metrics_map
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("internal: blueprint metrics not loaded"))?;
                let violations = engine.run(
                    assets.as_deref().ok_or_else(|| {
                        anyhow::anyhow!("internal: assets not loaded for lint check")
                    })?,
                    metrics,
                );
                let count = violations.len();
                let findings = violations
                    .iter()
                    .map(|v| {
                        let sev = match v.severity {
                            lint_engine::Severity::Error => "ERROR",
                            lint_engine::Severity::Warning => "WARN",
                        };
                        format!("{sev}  {}  {}", v.rule_id, v.asset_path.as_str())
                    })
                    .collect();
                CheckResult {
                    name,
                    passed: count == 0,
                    summary: format!("{count} violations"),
                    findings,
                }
            }
            "budget" => {
                let report = budget_tracker::check_budget(
                    assets.as_deref().ok_or_else(|| {
                        anyhow::anyhow!("internal: assets not loaded for budget check")
                    })?,
                    &config.budget,
                );
                let count = report.violations.len();
                let findings = report
                    .violations
                    .iter()
                    .map(|v| {
                        format!(
                            "{}: {}  {} > {}",
                            v.asset_type,
                            v.asset_path.as_str(),
                            crate::format_size(v.file_size),
                            crate::format_size(v.max_size),
                        )
                    })
                    .collect();
                CheckResult {
                    name,
                    passed: count == 0,
                    summary: format!("{count} over budget"),
                    findings,
                }
            }
            "duplicates" => {
                let assets_ref = assets.as_deref().ok_or_else(|| {
                    anyhow::anyhow!("internal: assets not loaded for duplicates check")
                })?;
                let by_name = duplicate_detector::detect_by_name(assets_ref);
                let tex_dup = duplicate_detector::detect_texture_duplicates(assets_ref);
                let tex_names: HashSet<&str> = tex_dup.iter().map(|g| g.name.as_str()).collect();
                let mut findings: Vec<String> = tex_dup
                    .iter()
                    .map(|g| format!("[texture-dup] {}", g.name))
                    .collect();
                for g in by_name
                    .iter()
                    .filter(|g| !tex_names.contains(g.name.as_str()))
                {
                    findings.push(format!("[same-name] {}", g.name));
                }
                let count = findings.len();
                CheckResult {
                    name,
                    passed: count == 0,
                    summary: format!("{count} groups"),
                    findings,
                }
            }
            _ => unreachable!(),
        };
        results.push(result);
    }

    let failed_count = results.iter().filter(|r| !r.passed).count();
    let total = results.len();
    let all_passed = failed_count == 0;

    match format {
        FormatKind::Json => {
            let output = CheckOutput {
                passed: all_passed,
                checks: results
                    .iter()
                    .map(|r| CheckResultJson {
                        name: r.name.to_owned(),
                        passed: r.passed,
                        findings: r.findings.clone(), // clone required: CheckResultJson owns the findings
                    })
                    .collect(),
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&output)
                    .context("Failed to serialize check output to JSON")?
            );
        }
        FormatKind::GithubActions | FormatKind::Text => {
            let name_width = active
                .iter()
                .map(|&n| check_display_name(n).len())
                .max()
                .unwrap_or(0);

            println!("Running project health checks...");
            println!();
            for r in &results {
                let icon = if r.passed { "\u{2713}" } else { "\u{2717}" };
                println!(
                    "  {} {:<width$}  \u{2014} {}",
                    icon,
                    check_display_name(r.name),
                    r.summary,
                    width = name_width,
                );
                for finding in &r.findings {
                    println!("      {finding}");
                }
            }
            println!();
            if all_passed {
                println!("Overall: PASSED ({total} of {total} checks passed)");
            } else {
                println!("Overall: FAILED ({failed_count} of {total} checks failed)");
            }
        }
    }

    if all_passed { Ok(0) } else { Ok(1) }
}

fn check_display_name(name: &str) -> &str {
    match name {
        "dead-assets" => "Dead assets",
        "cycles" => "Circular deps",
        "redirectors" => "Redirectors",
        "lint" => "Lint",
        "budget" => "Budget",
        "duplicates" => "Duplicates",
        _ => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::make_meta;
    use shared::{AssetPath, AssetType};

    #[test]
    fn handle_check_should_return_err_when_db_does_not_exist() {
        let db_path = std::env::temp_dir().join(format!(
            "uasset_lens_check159_missing_{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);

        let result = handle_check(Path::new("/proj"), &[], &[], &db_path, &FormatKind::Text);
        assert!(result.is_err(), "missing DB should return an error");
    }

    #[test]
    fn handle_check_should_return_0_when_db_is_empty() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_check159_empty_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_check(&dir, &[], &[], &db_path, &FormatKind::Text).unwrap();
        assert_eq!(result, 0, "empty DB — all checks pass");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_should_return_1_when_dead_asset_exists() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_check159_dead_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            // Texture2D with T_ prefix avoids naming-prefix lint violation.
            db.upsert_all(&[make_meta(
                "/Game/T_Orphan",
                dir.join("T_Orphan.uasset"),
                AssetType::Texture2D,
                4096,
                vec![],
            )])
            .unwrap();
        }

        let result = handle_check(&dir, &[], &[], &db_path, &FormatKind::Text).unwrap();
        assert_eq!(result, 1, "orphan asset triggers dead-assets check");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_should_return_1_when_cycle_exists() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_check159_cycle_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/BP_A",
                    dir.join("BP_A.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![AssetPath::new("/Game/BP_B").unwrap()],
                ),
                make_meta(
                    "/Game/BP_B",
                    dir.join("BP_B.uasset"),
                    AssetType::Blueprint,
                    1024,
                    vec![AssetPath::new("/Game/BP_A").unwrap()],
                ),
            ])
            .unwrap();
        }

        let result = handle_check(&dir, &[], &[], &db_path, &FormatKind::Text).unwrap();
        assert_eq!(result, 1, "A→B→A cycle triggers cycles check");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_json_should_contain_passed_and_checks_keys() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_check159_json_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        asset_db::AssetDb::open(&db_path).unwrap();

        // CheckOutput is the exact type handle_check serializes; verify its JSON schema.
        let json = serde_json::to_string(&CheckOutput {
            passed: true,
            checks: vec![],
        })
        .unwrap();
        assert!(
            json.contains("\"passed\""),
            "JSON must contain 'passed' key"
        );
        assert!(
            json.contains("\"checks\""),
            "JSON must contain 'checks' key"
        );

        let result = handle_check(&dir, &[], &[], &db_path, &FormatKind::Json).unwrap();
        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_only_filter_should_run_only_specified_checks() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_check159_only_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            // Orphan asset exists but --only cycles → dead-assets check not run.
            db.upsert_all(&[make_meta(
                "/Game/T_Orphan",
                dir.join("T_Orphan.uasset"),
                AssetType::Texture2D,
                4096,
                vec![],
            )])
            .unwrap();
        }

        let only = vec!["cycles".to_owned()];
        let result = handle_check(&dir, &only, &[], &db_path, &FormatKind::Text).unwrap();
        assert_eq!(result, 0, "--only cycles skips dead-assets");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_skip_filter_should_skip_specified_checks() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_check159_skip_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/T_Orphan",
                dir.join("T_Orphan.uasset"),
                AssetType::Texture2D,
                4096,
                vec![],
            )])
            .unwrap();
        }

        let skip = vec!["dead-assets".to_owned()];
        let result = handle_check(&dir, &[], &skip, &db_path, &FormatKind::Text).unwrap();
        assert_eq!(result, 0, "--skip dead-assets means orphan is ignored");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_should_return_err_for_unknown_only_check_name() {
        let db_path = std::env::temp_dir().join(format!(
            "uasset_lens_check159_unk_only_{}.db",
            std::process::id()
        ));
        let only = vec!["bogus".to_owned()];
        let result = handle_check(Path::new("/proj"), &only, &[], &db_path, &FormatKind::Text);
        assert!(result.is_err(), "unknown check name in --only should error");
    }

    #[test]
    fn handle_check_should_return_err_for_unknown_skip_check_name() {
        let db_path = std::env::temp_dir().join(format!(
            "uasset_lens_check159_unk_skip_{}.db",
            std::process::id()
        ));
        let skip = vec!["bogus".to_owned()];
        let result = handle_check(Path::new("/proj"), &[], &skip, &db_path, &FormatKind::Text);
        assert!(result.is_err(), "unknown check name in --skip should error");
    }
}
