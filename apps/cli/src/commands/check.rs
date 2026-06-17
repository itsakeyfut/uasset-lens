use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Context;

use crate::FormatKind;
use crate::config::{CheckSeverity, RulesConfig};

const ALL_CHECKS: &[&str] = &[
    "dead-assets",
    "cycles",
    "redirectors",
    "lint",
    "budget",
    "duplicates",
];

const CHECK_SAMPLE_LIMIT: usize = 5;

struct CheckResult {
    name: &'static str,
    severity: CheckSeverity,
    passed: bool,
    summary: String,
    findings: Vec<String>,
}

impl CheckResult {
    // A check blocks CI (exit 1) only when it is `Error` severity and has findings.
    // `Warn` findings are reported but do not fail the gate; `Off` checks never run.
    fn blocks(&self) -> bool {
        !self.passed && self.severity == CheckSeverity::Error
    }
}

#[derive(serde::Serialize)]
struct CheckResultJson {
    name: String,
    severity: &'static str,
    passed: bool,
    findings: Vec<String>,
}

// Built-in default severity for each check when no `[rules]` override is set, plus
// the mapping from check name to `[rules]` key. `lint` / `budget` are not gated by
// `[rules]` (they have their own severity model) and always block on findings.
fn effective_severity(name: &str, rules: &RulesConfig) -> CheckSeverity {
    match name {
        "dead-assets" => rules.dead_assets.unwrap_or(CheckSeverity::Warn),
        "cycles" => rules.circular_deps.unwrap_or(CheckSeverity::Error),
        "duplicates" => rules.duplicate_assets.unwrap_or(CheckSeverity::Warn),
        "redirectors" => rules.redirectors.unwrap_or(CheckSeverity::Warn),
        _ => CheckSeverity::Error,
    }
}

fn severity_label(severity: CheckSeverity) -> &'static str {
    match severity {
        CheckSeverity::Error => "error",
        CheckSeverity::Warn => "warn",
        CheckSeverity::Off => "off",
    }
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
    verbose: bool,
    db_path: &Path,
    cfg: &crate::config::ConfigFile,
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
        // `[rules] <check> = "off"` disables the check entirely (not run, not counted).
        .filter(|&n| effective_severity(n, &cfg.rules) != CheckSeverity::Off)
        .collect();

    let db = crate::open_db(db_path)?;

    let needs_graph = active
        .iter()
        .any(|&n| matches!(n, "dead-assets" | "cycles" | "redirectors"));
    let needs_assets = active
        .iter()
        .any(|&n| matches!(n, "lint" | "budget" | "duplicates"));

    let graph = if needs_graph {
        Some(crate::load_graph(&db, &cfg.scan.external_roots)?)
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
        let map: HashMap<uasset_lens_shared::AssetPath, uasset_lens_scanner::BlueprintMetrics> =
            rows.into_iter()
                .map(|row| {
                    (
                        row.asset_path,
                        uasset_lens_scanner::BlueprintMetrics {
                            node_count: row.node_count,
                            event_tick_count: row.event_tick_count,
                            cast_count: row.cast_count,
                            dependency_depth: row.dependency_depth,
                        },
                    )
                })
                .collect();
        let engine =
            uasset_lens_analysis::LintEngine::new(crate::lint_builder::build_lint_rules(&cfg.lint));
        (Some(map), Some(engine))
    } else {
        (None, None)
    };

    let mut results: Vec<CheckResult> = Vec::new();

    for &name in &active {
        let severity = effective_severity(name, &cfg.rules);
        let result = match name {
            "dead-assets" => {
                let g = graph.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("internal: graph not loaded for dead-assets check")
                })?;
                let dead = uasset_lens_dependency_graph::dead_assets::detect(
                    g,
                    uasset_lens_dependency_graph::dead_assets::DEFAULT_EXCLUDED_TYPES,
                );
                let count = dead.len();
                let findings = dead.iter().map(|p| p.as_str().to_owned()).collect();
                CheckResult {
                    name,
                    severity,
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
                    severity,
                    passed: count == 0,
                    summary: format!("{count} cycles"),
                    findings,
                }
            }
            "redirectors" => {
                let g = graph.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("internal: graph not loaded for redirectors check")
                })?;
                let paths = uasset_lens_dependency_graph::redirectors::detect(g);
                let count = paths.len();
                let findings = paths.iter().map(|p| p.as_str().to_owned()).collect();
                CheckResult {
                    name,
                    severity,
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
                            uasset_lens_analysis::Severity::Error => "ERROR",
                            uasset_lens_analysis::Severity::Warning => "WARN",
                        };
                        format!("{sev}  {}  {}", v.rule_id, v.asset_path.as_str())
                    })
                    .collect();
                CheckResult {
                    name,
                    severity,
                    passed: count == 0,
                    summary: format!("{count} violations"),
                    findings,
                }
            }
            "budget" => {
                let report = uasset_lens_analysis::check_budget(
                    assets.as_deref().ok_or_else(|| {
                        anyhow::anyhow!("internal: assets not loaded for budget check")
                    })?,
                    &cfg.budget,
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
                    severity,
                    passed: count == 0,
                    summary: format!("{count} over budget"),
                    findings,
                }
            }
            "duplicates" => {
                let assets_ref = assets.as_deref().ok_or_else(|| {
                    anyhow::anyhow!("internal: assets not loaded for duplicates check")
                })?;
                let by_name = uasset_lens_asset_db::duplicates::detect_by_name(assets_ref);
                let tex_dup =
                    uasset_lens_asset_db::duplicates::detect_texture_duplicates(assets_ref);
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
                    severity,
                    passed: count == 0,
                    summary: format!("{count} groups"),
                    findings,
                }
            }
            _ => unreachable!(),
        };
        results.push(result);
    }

    // Only `Error`-severity checks with findings fail the gate; `Warn` findings are
    // reported but non-blocking. The process exit code follows `gate_passed`.
    let blocking_count = results.iter().filter(|r| r.blocks()).count();
    let warning_count = results
        .iter()
        .filter(|r| !r.passed && r.severity == CheckSeverity::Warn)
        .count();
    let total = results.len();
    let gate_passed = blocking_count == 0;

    match format {
        FormatKind::Json => {
            let output = CheckOutput {
                passed: gate_passed,
                checks: results
                    .iter()
                    .map(|r| CheckResultJson {
                        name: r.name.to_owned(),
                        severity: severity_label(r.severity),
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
                let icon = if r.passed {
                    "\u{2713}" // ✓ no findings
                } else if r.severity == CheckSeverity::Error {
                    "\u{2717}" // ✗ blocking failure
                } else {
                    "\u{26a0}" // ⚠ warning (non-blocking)
                };
                println!(
                    "  {} {:<width$}  \u{2014} {}",
                    icon,
                    check_display_name(r.name),
                    r.summary,
                    width = name_width,
                );
                let sample_end = if verbose {
                    r.findings.len()
                } else {
                    r.findings.len().min(CHECK_SAMPLE_LIMIT)
                };
                for finding in &r.findings[..sample_end] {
                    println!("      {finding}");
                }
                if !verbose && r.findings.len() > CHECK_SAMPLE_LIMIT {
                    let remaining = r.findings.len() - CHECK_SAMPLE_LIMIT;
                    println!(
                        "      ... and {remaining} more. Run `uasset-lens {} {}` for the full list.",
                        check_full_command(r.name),
                        project_dir.display(),
                    );
                }
            }
            println!();
            if gate_passed && warning_count == 0 {
                println!("Overall: PASSED ({total} of {total} checks passed)");
            } else if gate_passed {
                println!("Overall: PASSED ({warning_count} warning(s), 0 blocking)");
            } else {
                println!("Overall: FAILED ({blocking_count} blocking, {warning_count} warning(s))");
            }
        }
    }

    if gate_passed { Ok(0) } else { Ok(1) }
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

fn check_full_command(name: &str) -> &str {
    match name {
        "dead-assets" => "dead-assets",
        "cycles" => "graph --cycles-only",
        "redirectors" => "redirectors",
        "lint" => "lint",
        "budget" => "budget",
        "duplicates" => "duplicates",
        _ => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{make_meta, test_db_in_tempdir};
    use uasset_lens_shared::{AssetPath, AssetType};

    fn cfg_with_rules(rules: crate::config::RulesConfig) -> crate::config::ConfigFile {
        crate::config::ConfigFile {
            rules,
            ..Default::default()
        }
    }

    #[test]
    fn handle_check_should_return_err_when_db_does_not_exist() {
        let (dir, db_path) = test_db_in_tempdir("check159_missing");
        let _ = std::fs::remove_dir_all(&dir);

        let result = handle_check(
            Path::new("/proj"),
            &[],
            &[],
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        );
        assert!(result.is_err(), "missing DB should return an error");
    }

    #[test]
    fn handle_check_should_return_0_when_db_is_empty() {
        let (dir, db_path) = test_db_in_tempdir("check159_empty");
        uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_check(
            &dir,
            &[],
            &[],
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "empty DB — all checks pass");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Texture2D with T_ prefix avoids naming-prefix lint violation; an unreferenced
    // orphan only triggers the dead-assets check.
    fn upsert_one_orphan(dir: &std::path::Path, db_path: &std::path::Path) {
        let mut db = uasset_lens_asset_db::AssetDb::open(db_path).unwrap();
        db.upsert_all(&[make_meta(
            "/Game/T_Orphan",
            dir.join("T_Orphan.uasset"),
            AssetType::Texture2D,
            4096,
            vec![],
        )])
        .unwrap();
    }

    #[test]
    fn handle_check_should_return_0_when_dead_assets_is_warn_by_default() {
        let (dir, db_path) = test_db_in_tempdir("check279_dead_warn");
        upsert_one_orphan(&dir, &db_path);

        // Default severity of dead-assets is "warn" → reported but non-blocking.
        let result = handle_check(
            &dir,
            &[],
            &[],
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "warn-severity dead assets must not block CI");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_should_return_1_when_dead_assets_severity_is_error() {
        let (dir, db_path) = test_db_in_tempdir("check279_dead_error");
        upsert_one_orphan(&dir, &db_path);

        let cfg = cfg_with_rules(RulesConfig {
            dead_assets: Some(CheckSeverity::Error),
            ..Default::default()
        });
        let result =
            handle_check(&dir, &[], &[], false, &db_path, &cfg, &FormatKind::Text).unwrap();
        assert_eq!(result, 1, "error-severity dead assets block CI");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_should_skip_dead_assets_when_severity_is_off() {
        let (dir, db_path) = test_db_in_tempdir("check279_dead_off");
        upsert_one_orphan(&dir, &db_path);

        // "off" disables the check entirely; even error-equivalent assets are ignored.
        let cfg = cfg_with_rules(RulesConfig {
            dead_assets: Some(CheckSeverity::Off),
            ..Default::default()
        });
        let result =
            handle_check(&dir, &[], &[], false, &db_path, &cfg, &FormatKind::Text).unwrap();
        assert_eq!(result, 0, "off-severity dead-assets check is skipped");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // BP_A→BP_B→BP_A: a referencing cycle. Neither node is dead (both referenced).
    fn upsert_cycle(dir: &std::path::Path, db_path: &std::path::Path) {
        let mut db = uasset_lens_asset_db::AssetDb::open(db_path).unwrap();
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

    #[test]
    fn handle_check_should_return_1_when_cycle_exists() {
        let (dir, db_path) = test_db_in_tempdir("check159_cycle");
        upsert_cycle(&dir, &db_path);

        // circular-deps defaults to "error" → blocks CI.
        let result = handle_check(
            &dir,
            &[],
            &[],
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 1, "A→B→A cycle triggers cycles check");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_should_return_0_when_circular_deps_is_warn() {
        let (dir, db_path) = test_db_in_tempdir("check279_cycle_warn");
        upsert_cycle(&dir, &db_path);

        // circular-deps = "warn" → cycle reported as a warning, does not block CI.
        let cfg = cfg_with_rules(RulesConfig {
            circular_deps: Some(CheckSeverity::Warn),
            ..Default::default()
        });
        let result =
            handle_check(&dir, &[], &[], false, &db_path, &cfg, &FormatKind::Text).unwrap();
        assert_eq!(result, 0, "warn-severity cycle findings must not block CI");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_json_should_contain_passed_and_checks_keys() {
        let (dir, db_path) = test_db_in_tempdir("check159_json");
        uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();

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

        let result = handle_check(
            &dir,
            &[],
            &[],
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Json,
        )
        .unwrap();
        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_only_filter_should_run_only_specified_checks() {
        let (dir, db_path) = test_db_in_tempdir("check159_only");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
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
        let result = handle_check(
            &dir,
            &only,
            &[],
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "--only cycles skips dead-assets");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_skip_filter_should_skip_specified_checks() {
        let (dir, db_path) = test_db_in_tempdir("check159_skip");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
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
        let result = handle_check(
            &dir,
            &[],
            &skip,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "--skip dead-assets means orphan is ignored");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_should_return_err_for_unknown_only_check_name() {
        let (dir, db_path) = test_db_in_tempdir("check159_unk_only");
        let only = vec!["bogus".to_owned()];
        let result = handle_check(
            Path::new("/proj"),
            &only,
            &[],
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        );
        assert!(result.is_err(), "unknown check name in --only should error");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_should_return_err_for_unknown_skip_check_name() {
        let (dir, db_path) = test_db_in_tempdir("check159_unk_skip");
        let skip = vec!["bogus".to_owned()];
        let result = handle_check(
            Path::new("/proj"),
            &[],
            &skip,
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
        );
        assert!(result.is_err(), "unknown check name in --skip should error");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_text_should_return_1_and_not_panic_with_many_findings_when_not_verbose() {
        let (dir, db_path) = test_db_in_tempdir("check236_truncate");
        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            // 6 orphans exceed CHECK_SAMPLE_LIMIT (5) → truncation path exercised
            db.upsert_all(&[
                make_meta(
                    "/Game/T_A",
                    dir.join("T_A.uasset"),
                    AssetType::Texture2D,
                    512,
                    vec![],
                ),
                make_meta(
                    "/Game/T_B",
                    dir.join("T_B.uasset"),
                    AssetType::Texture2D,
                    512,
                    vec![],
                ),
                make_meta(
                    "/Game/T_C",
                    dir.join("T_C.uasset"),
                    AssetType::Texture2D,
                    512,
                    vec![],
                ),
                make_meta(
                    "/Game/T_D",
                    dir.join("T_D.uasset"),
                    AssetType::Texture2D,
                    512,
                    vec![],
                ),
                make_meta(
                    "/Game/T_E",
                    dir.join("T_E.uasset"),
                    AssetType::Texture2D,
                    512,
                    vec![],
                ),
                make_meta(
                    "/Game/T_F",
                    dir.join("T_F.uasset"),
                    AssetType::Texture2D,
                    512,
                    vec![],
                ),
            ])
            .unwrap();
        }
        // dead-assets = "error" so the 6 findings block CI (exercises the failing path).
        let cfg = cfg_with_rules(RulesConfig {
            dead_assets: Some(CheckSeverity::Error),
            ..Default::default()
        });
        let result =
            handle_check(&dir, &[], &[], false, &db_path, &cfg, &FormatKind::Text).unwrap();
        assert_eq!(result, 1, "6 dead assets → check fails");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_text_should_return_1_and_not_panic_with_many_findings_when_verbose() {
        let (dir, db_path) = test_db_in_tempdir("check236_verbose");
        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/T_A",
                    dir.join("T_A.uasset"),
                    AssetType::Texture2D,
                    512,
                    vec![],
                ),
                make_meta(
                    "/Game/T_B",
                    dir.join("T_B.uasset"),
                    AssetType::Texture2D,
                    512,
                    vec![],
                ),
                make_meta(
                    "/Game/T_C",
                    dir.join("T_C.uasset"),
                    AssetType::Texture2D,
                    512,
                    vec![],
                ),
                make_meta(
                    "/Game/T_D",
                    dir.join("T_D.uasset"),
                    AssetType::Texture2D,
                    512,
                    vec![],
                ),
                make_meta(
                    "/Game/T_E",
                    dir.join("T_E.uasset"),
                    AssetType::Texture2D,
                    512,
                    vec![],
                ),
                make_meta(
                    "/Game/T_F",
                    dir.join("T_F.uasset"),
                    AssetType::Texture2D,
                    512,
                    vec![],
                ),
            ])
            .unwrap();
        }
        let cfg = cfg_with_rules(RulesConfig {
            dead_assets: Some(CheckSeverity::Error),
            ..Default::default()
        });
        let result = handle_check(&dir, &[], &[], true, &db_path, &cfg, &FormatKind::Text).unwrap();
        assert_eq!(result, 1, "verbose=true, 6 dead assets → check fails");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
