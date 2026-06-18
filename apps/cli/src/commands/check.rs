use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Context;

use crate::FormatKind;
use crate::config::{CheckSeverity, RulesConfig};

/// Severity as serialized in the violation baseline. Only `Error` participates in
/// regression detection; `Warn` is recorded but never gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum ViolationSeverity {
    Error,
    Warn,
}

/// A single check finding in structured form, used for the baseline JSON.
/// `file` is intentionally omitted to keep comparisons path-stable across machines.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Violation {
    severity: ViolationSeverity,
    rule: String,
    asset_path: String,
    message: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct BaselineSummary {
    errors: usize,
    warnings: usize,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct BaselineDoc {
    version: u32,
    git_commit: String,
    summary: BaselineSummary,
    violations: Vec<Violation>,
}

fn to_violation_severity(s: CheckSeverity) -> ViolationSeverity {
    match s {
        CheckSeverity::Error => ViolationSeverity::Error,
        _ => ViolationSeverity::Warn,
    }
}

fn violation_key(v: &Violation) -> (String, String) {
    (v.rule.clone(), v.asset_path.clone())
}

/// New error-severity violations present in `current` but not in `baseline`,
/// matched by (rule, asset_path). Warnings are excluded on both sides.
fn compute_regressions(baseline: &[Violation], current: &[Violation]) -> Vec<Violation> {
    let baseline_errors: HashSet<(String, String)> = baseline
        .iter()
        .filter(|v| v.severity == ViolationSeverity::Error)
        .map(violation_key)
        .collect();
    current
        .iter()
        .filter(|v| v.severity == ViolationSeverity::Error)
        .filter(|v| !baseline_errors.contains(&violation_key(v)))
        .cloned()
        .collect()
}

fn load_baseline(path: &Path) -> anyhow::Result<BaselineDoc> {
    let s = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read baseline file: {}", path.display()))?;
    serde_json::from_str(&s).with_context(|| format!("invalid baseline JSON: {}", path.display()))
}

fn save_baseline_file(
    path: &Path,
    project_dir: &Path,
    violations: Vec<Violation>,
    errors: usize,
    warnings: usize,
) -> anyhow::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).with_context(|| {
            format!("failed to create baseline directory: {}", parent.display())
        })?;
    }
    let doc = BaselineDoc {
        version: 1,
        git_commit: git_commit(project_dir),
        summary: BaselineSummary { errors, warnings },
        violations,
    };
    let json = serde_json::to_string_pretty(&doc).context("failed to serialize baseline JSON")?;
    std::fs::write(path, json)
        .with_context(|| format!("failed to write baseline file: {}", path.display()))?;
    eprintln!("Saved baseline to {}", path.display());
    Ok(())
}

/// `git rev-parse HEAD` in `project_dir`; empty string if git is unavailable or fails.
fn git_commit(project_dir: &Path) -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(project_dir)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .unwrap_or_default()
}

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
    violations: Vec<Violation>,
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

/// Runs an mtime delta scan before checks unless `skip_scan` is set, so `check` works
/// without a separate `scan`. Output is quiet (stderr progress only, no stdout report) to
/// keep `check`'s own stdout clean; stale records are auto-removed (equivalent to `scan -y`).
pub(crate) fn auto_scan(
    skip_scan: bool,
    project_dir: &Path,
    db_path: &Path,
    cfg: &crate::config::ConfigFile,
) -> anyhow::Result<()> {
    if skip_scan {
        return Ok(());
    }
    // The pre-check scan is quiet (no stdout), so its format is irrelevant — pass Text to avoid
    // the SARIF guard in `handle_scan`. Scan's exit code (e.g. stale-removed → 1) is discarded;
    // `check` computes its own exit code from the check results.
    crate::commands::scan::handle_scan(
        project_dir,
        db_path,
        &FormatKind::Text,
        cfg,
        &crate::commands::scan::ScanOptions {
            full_scan: false,
            diff: false,
            yes: true,
            save_baseline: None,
            diff_from: None,
            quiet: true,
            no_progress: true,
        },
    )?;
    Ok(())
}

// Thin 7-arg shim retained for the existing test suite; production dispatch calls
// `handle_check_with_baseline` directly.
#[cfg(test)]
pub fn handle_check(
    project_dir: &Path,
    only: &[String],
    skip: &[String],
    verbose: bool,
    db_path: &Path,
    cfg: &crate::config::ConfigFile,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    handle_check_with_baseline(
        project_dir,
        only,
        skip,
        verbose,
        db_path,
        cfg,
        format,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn handle_check_with_baseline(
    project_dir: &Path,
    only: &[String],
    skip: &[String],
    verbose: bool,
    db_path: &Path,
    cfg: &crate::config::ConfigFile,
    format: &FormatKind,
    save_baseline: Option<&Path>,
    diff_from: Option<&Path>,
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

    // SARIF resolves a file path for every finding (including graph-only checks), so it needs the
    // asset table even when no asset-based check runs.
    let assets = if needs_assets || matches!(format, FormatKind::Sarif) {
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
                let violations = dead
                    .iter()
                    .map(|p| Violation {
                        severity: to_violation_severity(severity),
                        rule: "dead-assets".to_owned(),
                        asset_path: p.as_str().to_owned(),
                        message: "unreferenced asset".to_owned(),
                    })
                    .collect();
                CheckResult {
                    name,
                    severity,
                    passed: count == 0,
                    summary: format!("{count} dead assets"),
                    findings,
                    violations,
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
                // One violation per cycle member so a newly-cyclic asset is detected
                // as a (rule, asset_path) regression.
                let violations = cycles
                    .iter()
                    .flat_map(|scc| {
                        let mut parts: Vec<String> =
                            scc.iter().map(|p| p.as_str().to_owned()).collect();
                        if let Some(first) = parts.first().cloned() {
                            parts.push(first);
                        }
                        let cycle_str = parts.join(" \u{2192} ");
                        scc.iter()
                            .map(|node| Violation {
                                severity: to_violation_severity(severity),
                                rule: "circular-deps".to_owned(),
                                asset_path: node.as_str().to_owned(),
                                message: cycle_str.clone(),
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect();
                CheckResult {
                    name,
                    severity,
                    passed: count == 0,
                    summary: format!("{count} cycles"),
                    findings,
                    violations,
                }
            }
            "redirectors" => {
                let g = graph.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("internal: graph not loaded for redirectors check")
                })?;
                let paths = uasset_lens_dependency_graph::redirectors::detect(g);
                let count = paths.len();
                let findings = paths.iter().map(|p| p.as_str().to_owned()).collect();
                let violations = paths
                    .iter()
                    .map(|p| Violation {
                        severity: to_violation_severity(severity),
                        rule: "redirectors".to_owned(),
                        asset_path: p.as_str().to_owned(),
                        message: "unresolved redirector".to_owned(),
                    })
                    .collect();
                CheckResult {
                    name,
                    severity,
                    passed: count == 0,
                    summary: format!("{count} redirectors"),
                    findings,
                    violations,
                }
            }
            "lint" => {
                let engine = lint_engine
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("internal: lint engine not initialized"))?;
                let metrics = bp_metrics_map
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("internal: blueprint metrics not loaded"))?;
                let lint_violations = engine.run(
                    assets.as_deref().ok_or_else(|| {
                        anyhow::anyhow!("internal: assets not loaded for lint check")
                    })?,
                    metrics,
                );
                let count = lint_violations.len();
                let findings = lint_violations
                    .iter()
                    .map(|v| {
                        let sev = match v.severity {
                            uasset_lens_analysis::Severity::Error => "ERROR",
                            uasset_lens_analysis::Severity::Warning => "WARN",
                        };
                        format!("{sev}  {}  {}", v.rule_id, v.asset_path.as_str())
                    })
                    .collect();
                let violations = lint_violations
                    .iter()
                    .map(|v| Violation {
                        severity: match v.severity {
                            uasset_lens_analysis::Severity::Error => ViolationSeverity::Error,
                            uasset_lens_analysis::Severity::Warning => ViolationSeverity::Warn,
                        },
                        rule: v.rule_id.clone(),
                        asset_path: v.asset_path.as_str().to_owned(),
                        message: v.message.clone(),
                    })
                    .collect();
                // The lint check blocks only when a per-rule-severity Error violation
                // exists; all-Warning lint output is non-blocking ([lint] severity).
                let has_error = lint_violations
                    .iter()
                    .any(|v| v.severity == uasset_lens_analysis::Severity::Error);
                CheckResult {
                    name,
                    severity: if has_error {
                        CheckSeverity::Error
                    } else {
                        CheckSeverity::Warn
                    },
                    passed: count == 0,
                    summary: format!("{count} violations"),
                    findings,
                    violations,
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
                let violations = report
                    .violations
                    .iter()
                    .map(|v| Violation {
                        severity: to_violation_severity(severity),
                        rule: crate::budget_rule_id(&v.asset_type),
                        asset_path: v.asset_path.as_str().to_owned(),
                        message: format!(
                            "{} > {}",
                            crate::format_size(v.file_size),
                            crate::format_size(v.max_size),
                        ),
                    })
                    .collect();
                CheckResult {
                    name,
                    severity,
                    passed: count == 0,
                    summary: format!("{count} over budget"),
                    findings,
                    violations,
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
                let mut violations: Vec<Violation> = tex_dup
                    .iter()
                    .map(|g| Violation {
                        severity: to_violation_severity(severity),
                        rule: "duplicate-assets".to_owned(),
                        asset_path: g.name.clone(),
                        message: format!("[texture-dup] {}", g.name),
                    })
                    .collect();
                for g in by_name
                    .iter()
                    .filter(|g| !tex_names.contains(g.name.as_str()))
                {
                    findings.push(format!("[same-name] {}", g.name));
                    violations.push(Violation {
                        severity: to_violation_severity(severity),
                        rule: "duplicate-assets".to_owned(),
                        asset_path: g.name.clone(),
                        message: format!("[same-name] {}", g.name),
                    });
                }
                let count = findings.len();
                CheckResult {
                    name,
                    severity,
                    passed: count == 0,
                    summary: format!("{count} groups"),
                    findings,
                    violations,
                }
            }
            _ => unreachable!(),
        };
        results.push(result);
    }

    let all_violations: Vec<Violation> = results
        .iter()
        .flat_map(|r| r.violations.iter().cloned())
        .collect();
    let error_total = all_violations
        .iter()
        .filter(|v| v.severity == ViolationSeverity::Error)
        .count();
    let warning_total = all_violations.len() - error_total;

    // CLI `--diff-from` (CWD-relative) takes priority. A relative `[check] baseline_path`
    // from config is resolved against the project dir (where .uasset-lens.toml lives) so a
    // committed, shared baseline path is portable regardless of the working directory.
    let config_baseline = cfg.check.baseline_path.as_ref().map(|p| {
        if p.is_relative() {
            project_dir.join(p)
        } else {
            p.clone()
        }
    });
    let diff_from = diff_from.map(Path::to_path_buf).or(config_baseline);

    // Load baseline and compute regressions up front so a missing/invalid baseline
    // fails fast (exit 2) before any check output is written. `--diff-from` replaces
    // the normal gate with regression-only gating.
    let regressions: Option<Vec<Violation>> = match &diff_from {
        Some(path) => {
            let baseline = load_baseline(path)?;
            Some(compute_regressions(&baseline.violations, &all_violations))
        }
        None => None,
    };

    // Only `Error`-severity checks with findings fail the gate; `Warn` findings are
    // reported but non-blocking. The process exit code follows `gate_passed`.
    let blocking_count = results.iter().filter(|r| r.blocks()).count();
    let warning_count = results
        .iter()
        .filter(|r| !r.passed && r.severity == CheckSeverity::Warn)
        .count();
    let total = results.len();
    let gate_passed = blocking_count == 0;
    // `--diff-from` re-bases the gate onto regressions; the serialized `passed` and the
    // process exit code must derive from the same effective gate.
    let effective_passed = match &regressions {
        Some(regs) => regs.is_empty(),
        None => gate_passed,
    };

    match format {
        FormatKind::Sarif => {
            let findings = build_check_findings(
                &all_violations,
                assets.as_deref().unwrap_or(&[]),
                project_dir,
            );
            println!("{}", crate::sarif::to_sarif_json(&findings)?);
        }
        FormatKind::Json => {
            let output = CheckOutput {
                passed: effective_passed,
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
            // Keep stdout a single JSON value; report the regression verdict on stderr.
            if let Some(regs) = &regressions {
                eprintln!(
                    "Regression check vs baseline: {} new error violation(s)",
                    regs.len()
                );
            }
        }
        FormatKind::GithubActions | FormatKind::Text => {
            let name_width = active
                .iter()
                .map(|&n| check_display_name(n).len())
                .max()
                .unwrap_or(0);

            // Text caps each check at 5 items; verbose and github-actions show everything.
            let full_output = is_full_output(verbose, format);

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
                for line in finding_lines(
                    &r.findings,
                    full_output,
                    check_full_command(r.name),
                    project_dir,
                ) {
                    println!("{line}");
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
            if let Some(regs) = &regressions {
                println!();
                if regs.is_empty() {
                    println!("Regression check: PASSED (no new error violations vs baseline)");
                } else {
                    println!(
                        "Regression check: FAILED ({} new error violation(s) vs baseline)",
                        regs.len()
                    );
                    let end = if full_output {
                        regs.len()
                    } else {
                        regs.len().min(CHECK_SAMPLE_LIMIT)
                    };
                    for v in &regs[..end] {
                        println!("      {}  {}", v.rule, v.asset_path);
                    }
                    if !full_output && regs.len() > CHECK_SAMPLE_LIMIT {
                        println!("      ... and {} more.", regs.len() - CHECK_SAMPLE_LIMIT);
                    }
                }
            }
        }
    }

    let exit = if effective_passed { 0 } else { 1 };

    if let Some(path) = save_baseline {
        save_baseline_file(
            path,
            project_dir,
            all_violations,
            error_total,
            warning_total,
        )?;
    }

    Ok(exit)
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

/// Converts the aggregated check violations into SARIF findings: severity → level, and each
/// `asset_path` resolved to a project-relative file uri via the asset table (None when the
/// violation has no backing file, e.g. a duplicate group name).
fn build_check_findings(
    violations: &[Violation],
    assets: &[uasset_lens_asset_db::AssetRecord],
    project_dir: &Path,
) -> Vec<crate::sarif::SarifFinding> {
    let path_lookup: HashMap<&str, &Path> = assets
        .iter()
        .map(|r| (r.asset_path.as_str(), r.file_path.as_path()))
        .collect();
    violations
        .iter()
        .map(|v| crate::sarif::SarifFinding {
            rule_id: v.rule.clone(),
            level: match v.severity {
                ViolationSeverity::Error => crate::sarif::SarifLevel::Error,
                ViolationSeverity::Warn => crate::sarif::SarifLevel::Warning,
            },
            message: v.message.clone(),
            uri: path_lookup
                .get(v.asset_path.as_str())
                .map(|p| crate::rel_path_for_annotation(p, project_dir)),
        })
        .collect()
}

// The text output caps each check at `CHECK_SAMPLE_LIMIT` items; `--verbose` and the
// github-actions format always print everything (CI annotation streams must not truncate).
fn is_full_output(verbose: bool, format: &FormatKind) -> bool {
    verbose || matches!(format, FormatKind::GithubActions)
}

/// Renders one check's finding lines: up to `CHECK_SAMPLE_LIMIT` items, then a
/// "... and N more" line, unless `full_output` shows all items with no truncation line.
fn finding_lines(
    findings: &[String],
    full_output: bool,
    full_command: &str,
    project_dir: &Path,
) -> Vec<String> {
    let end = if full_output {
        findings.len()
    } else {
        findings.len().min(CHECK_SAMPLE_LIMIT)
    };
    let mut lines: Vec<String> = findings[..end]
        .iter()
        .map(|f| format!("      {f}"))
        .collect();
    if !full_output && findings.len() > CHECK_SAMPLE_LIMIT {
        let remaining = findings.len() - CHECK_SAMPLE_LIMIT;
        lines.push(format!(
            "      ... and {remaining} more. Run `uasset-lens {full_command} {}` for the full list.",
            project_dir.display(),
        ));
    }
    lines
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

    fn cfg_with_naming_severity(severity: CheckSeverity) -> crate::config::ConfigFile {
        crate::config::ConfigFile {
            lint: crate::config::LintConfig {
                naming: crate::config::LintNamingConfig {
                    severity: Some(severity),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        }
    }

    // A referenced, mis-named texture: triggers a naming lint violation but is not a
    // dead asset (BP_Owner references it). Lets the lint check be exercised in isolation.
    fn upsert_misnamed_referenced_texture(dir: &std::path::Path, db_path: &std::path::Path) {
        let mut db = uasset_lens_asset_db::AssetDb::open(db_path).unwrap();
        db.upsert_all(&[
            make_meta(
                "/Game/BP_Owner",
                dir.join("BP_Owner.uasset"),
                AssetType::Blueprint,
                1024,
                vec![AssetPath::new("/Game/Rock").unwrap()],
            ),
            make_meta(
                "/Game/Rock", // mis-named Texture2D (should start with T_)
                dir.join("Rock.uasset"),
                AssetType::Texture2D,
                512,
                vec![],
            ),
        ])
        .unwrap();
    }

    #[test]
    fn handle_check_should_not_block_on_naming_warning_by_default() {
        let (dir, db_path) = test_db_in_tempdir("check280_naming_warn");
        upsert_misnamed_referenced_texture(&dir, &db_path);

        // Naming defaults to "warn"; BP_Owner is a dead asset (also "warn" by default).
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
        assert_eq!(result, 0, "default naming warnings must not block CI");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_should_block_when_naming_severity_is_error() {
        let (dir, db_path) = test_db_in_tempdir("check280_naming_error");
        upsert_misnamed_referenced_texture(&dir, &db_path);

        let cfg = cfg_with_naming_severity(CheckSeverity::Error);
        let result =
            handle_check(&dir, &[], &[], false, &db_path, &cfg, &FormatKind::Text).unwrap();
        assert_eq!(
            result, 1,
            "naming.severity = error makes the lint check block"
        );
        let _ = std::fs::remove_dir_all(&dir);
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

    // --- #277: violation baseline / regression detection ---

    fn err_violation(rule: &str, asset_path: &str) -> Violation {
        Violation {
            severity: ViolationSeverity::Error,
            rule: rule.to_owned(),
            asset_path: asset_path.to_owned(),
            message: String::new(),
        }
    }

    fn write_baseline(path: &std::path::Path, violations: Vec<Violation>) {
        let errors = violations
            .iter()
            .filter(|v| v.severity == ViolationSeverity::Error)
            .count();
        let doc = BaselineDoc {
            version: 1,
            git_commit: String::new(),
            summary: BaselineSummary {
                errors,
                warnings: violations.len() - errors,
            },
            violations,
        };
        std::fs::write(path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
    }

    #[test]
    fn handle_check_diff_from_should_exit_0_when_no_new_regressions() {
        let (dir, db_path) = test_db_in_tempdir("check277_no_regress");
        upsert_misnamed_referenced_texture(&dir, &db_path);
        let baseline = dir.join("baseline.json");
        // The pre-existing naming error is in the baseline → not a regression.
        write_baseline(
            &baseline,
            vec![err_violation("naming/prefix", "/Game/Rock")],
        );

        let cfg = cfg_with_naming_severity(CheckSeverity::Error);
        let result = handle_check_with_baseline(
            &dir,
            &[],
            &[],
            false,
            &db_path,
            &cfg,
            &FormatKind::Text,
            None,
            Some(&baseline),
        )
        .unwrap();
        // Normal gate would exit 1 (error violation); --diff-from gates only on regressions.
        assert_eq!(result, 0, "pre-existing baseline violation must not fail");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_diff_from_should_exit_1_when_new_error_violation_not_in_baseline() {
        let (dir, db_path) = test_db_in_tempdir("check277_new_error");
        upsert_misnamed_referenced_texture(&dir, &db_path);
        let baseline = dir.join("baseline.json");
        write_baseline(&baseline, vec![]); // empty baseline → the naming error is new

        let cfg = cfg_with_naming_severity(CheckSeverity::Error);
        let result = handle_check_with_baseline(
            &dir,
            &[],
            &[],
            false,
            &db_path,
            &cfg,
            &FormatKind::Text,
            None,
            Some(&baseline),
        )
        .unwrap();
        assert_eq!(result, 1, "a new error violation is a regression");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_diff_from_should_exit_0_when_new_warning_violation() {
        let (dir, db_path) = test_db_in_tempdir("check277_new_warn");
        upsert_misnamed_referenced_texture(&dir, &db_path);
        let baseline = dir.join("baseline.json");
        write_baseline(&baseline, vec![]);

        // Default config: naming is "warn"; warnings never trigger a regression.
        let result = handle_check_with_baseline(
            &dir,
            &[],
            &[],
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
            None,
            Some(&baseline),
        )
        .unwrap();
        assert_eq!(result, 0, "new warning violations never gate");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_diff_from_should_exit_0_when_baseline_violation_resolved() {
        let (dir, db_path) = test_db_in_tempdir("check277_resolved");
        // Correctly-named orphan: no naming/budget/cycle errors at all.
        upsert_one_orphan(&dir, &db_path);
        let baseline = dir.join("baseline.json");
        // Baseline had an error that no longer exists in the current run.
        write_baseline(
            &baseline,
            vec![err_violation("naming/prefix", "/Game/Gone")],
        );

        let result = handle_check_with_baseline(
            &dir,
            &[],
            &[],
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
            None,
            Some(&baseline),
        )
        .unwrap();
        assert_eq!(
            result, 0,
            "a resolved baseline violation is not a regression"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_diff_from_should_err_when_baseline_file_missing() {
        let (dir, db_path) = test_db_in_tempdir("check277_missing_baseline");
        upsert_one_orphan(&dir, &db_path);
        let missing = dir.join("does-not-exist.json");

        let result = handle_check_with_baseline(
            &dir,
            &[],
            &[],
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
            None,
            Some(&missing),
        );
        assert!(result.is_err(), "missing baseline → error (exit 2)");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_save_baseline_should_write_valid_json_with_violations() {
        let (dir, db_path) = test_db_in_tempdir("check277_save");
        upsert_misnamed_referenced_texture(&dir, &db_path);
        let out = dir.join("nested").join("baseline.json");

        let cfg = cfg_with_naming_severity(CheckSeverity::Error);
        let _ = handle_check_with_baseline(
            &dir,
            &[],
            &[],
            false,
            &db_path,
            &cfg,
            &FormatKind::Text,
            Some(&out),
            None,
        )
        .unwrap();

        let doc: BaselineDoc =
            serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(doc.version, 1);
        assert!(doc.summary.errors >= 1);
        assert!(
            doc.violations.iter().any(|v| v.rule == "naming/prefix"
                && v.asset_path == "/Game/Rock"
                && v.severity == ViolationSeverity::Error),
            "saved baseline must contain the naming error for /Game/Rock"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_should_save_and_diff_when_both_flags_given() {
        let (dir, db_path) = test_db_in_tempdir("check277_save_and_diff");
        upsert_misnamed_referenced_texture(&dir, &db_path);
        let baseline = dir.join("baseline.json");
        write_baseline(&baseline, vec![]); // empty → current error is a regression
        let saved = dir.join("saved.json");

        let cfg = cfg_with_naming_severity(CheckSeverity::Error);
        let result = handle_check_with_baseline(
            &dir,
            &[],
            &[],
            false,
            &db_path,
            &cfg,
            &FormatKind::Text,
            Some(&saved),
            Some(&baseline),
        )
        .unwrap();
        assert_eq!(result, 1, "regression exits 1 even when also saving");
        let doc: BaselineDoc =
            serde_json::from_str(&std::fs::read_to_string(&saved).unwrap()).unwrap();
        assert!(
            doc.violations.iter().any(|v| v.rule == "naming/prefix"),
            "the new baseline is written alongside the diff"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_diff_from_should_exit_1_when_new_cycle_introduced() {
        let (dir, db_path) = test_db_in_tempdir("check277_new_cycle");
        upsert_cycle(&dir, &db_path); // BP_A↔BP_B; circular-deps defaults to error
        let baseline = dir.join("baseline.json");
        write_baseline(&baseline, vec![]); // empty → the new cycle is a regression

        // Exercises the non-lint (cycles) violation path that the "all 6 checks"
        // baseline scope exists to catch.
        let result = handle_check_with_baseline(
            &dir,
            &[],
            &[],
            false,
            &db_path,
            &Default::default(),
            &FormatKind::Text,
            None,
            Some(&baseline),
        )
        .unwrap();
        assert_eq!(result, 1, "a newly-introduced cycle is an error regression");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_diff_from_json_should_exit_0_when_no_new_regressions() {
        let (dir, db_path) = test_db_in_tempdir("check277_json_no_regress");
        upsert_misnamed_referenced_texture(&dir, &db_path);
        let baseline = dir.join("baseline.json");
        write_baseline(
            &baseline,
            vec![err_violation("naming/prefix", "/Game/Rock")],
        );

        // JSON path: the serialized `passed` and the exit code share one effective gate,
        // so a pre-existing (non-regressing) error must still exit 0.
        let cfg = cfg_with_naming_severity(CheckSeverity::Error);
        let result = handle_check_with_baseline(
            &dir,
            &[],
            &[],
            false,
            &db_path,
            &cfg,
            &FormatKind::Json,
            None,
            Some(&baseline),
        )
        .unwrap();
        assert_eq!(result, 0, "JSON mode regression gate matches text mode");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- #281: [check] baseline_path config default for --diff-from ---

    fn cfg_naming_error_with_baseline(path: &std::path::Path) -> crate::config::ConfigFile {
        crate::config::ConfigFile {
            lint: crate::config::LintConfig {
                naming: crate::config::LintNamingConfig {
                    severity: Some(CheckSeverity::Error),
                    ..Default::default()
                },
                ..Default::default()
            },
            check: crate::config::CheckSectionConfig {
                baseline_path: Some(path.to_path_buf()),
            },
            ..Default::default()
        }
    }

    #[test]
    fn handle_check_should_auto_diff_when_config_baseline_path_set() {
        let (dir, db_path) = test_db_in_tempdir("check281_auto_diff");
        upsert_misnamed_referenced_texture(&dir, &db_path);
        let baseline = dir.join("baseline.json");
        // Baseline already contains the current error → no regression → exit 0, even though
        // the normal gate (naming severity = error) would exit 1. Discriminating.
        write_baseline(
            &baseline,
            vec![err_violation("naming/prefix", "/Game/Rock")],
        );

        let cfg = cfg_naming_error_with_baseline(&baseline);
        let result = handle_check_with_baseline(
            &dir,
            &[],
            &[],
            false,
            &db_path,
            &cfg,
            &FormatKind::Text,
            None,
            None, // no CLI --diff-from → config baseline_path is used
        )
        .unwrap();
        assert_eq!(
            result, 0,
            "config baseline_path triggers auto-diff and replaces the normal gate"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_cli_diff_from_should_override_config_baseline_path() {
        let (dir, db_path) = test_db_in_tempdir("check281_cli_override");
        upsert_misnamed_referenced_texture(&dir, &db_path);
        let config_baseline = dir.join("config_baseline.json");
        write_baseline(
            &config_baseline,
            vec![err_violation("naming/prefix", "/Game/Rock")], // would exit 0 if used
        );
        let cli_baseline = dir.join("cli_baseline.json");
        write_baseline(&cli_baseline, vec![]); // empty → error is a new regression → exit 1

        let cfg = cfg_naming_error_with_baseline(&config_baseline);
        let result = handle_check_with_baseline(
            &dir,
            &[],
            &[],
            false,
            &db_path,
            &cfg,
            &FormatKind::Text,
            None,
            Some(&cli_baseline),
        )
        .unwrap();
        assert_eq!(result, 1, "CLI --diff-from overrides config baseline_path");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_should_err_when_config_baseline_path_missing() {
        let (dir, db_path) = test_db_in_tempdir("check281_config_missing");
        upsert_one_orphan(&dir, &db_path);
        let missing = dir.join("does-not-exist.json");

        // Config-sourced missing baseline behaves like an explicit --diff-from: exit 2.
        let cfg = cfg_naming_error_with_baseline(&missing);
        let result = handle_check_with_baseline(
            &dir,
            &[],
            &[],
            false,
            &db_path,
            &cfg,
            &FormatKind::Text,
            None,
            None,
        );
        assert!(
            result.is_err(),
            "missing config baseline → error (exit 2), same as --diff-from"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_should_not_auto_diff_when_check_section_absent() {
        let (dir, db_path) = test_db_in_tempdir("check281_no_section");
        upsert_misnamed_referenced_texture(&dir, &db_path);

        // Default config has no [check] baseline_path → no auto-diff. The naming error then
        // falls through to the normal gate (exit 1) — not exit 2 from a phantom default path.
        let cfg = cfg_with_naming_severity(CheckSeverity::Error);
        let result = handle_check_with_baseline(
            &dir,
            &[],
            &[],
            false,
            &db_path,
            &cfg,
            &FormatKind::Text,
            None,
            None,
        )
        .unwrap();
        assert_eq!(result, 1, "no baseline_path → normal gate, no auto-diff");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_check_should_resolve_relative_config_baseline_against_project_dir() {
        let (dir, db_path) = test_db_in_tempdir("check281_relative_resolve");
        upsert_misnamed_referenced_texture(&dir, &db_path);
        // Baseline lives under <project_dir>/sub/, referenced by a *relative* config path.
        // The test CWD is the crate dir, so CWD-relative resolution would NOT find it.
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        write_baseline(
            &sub.join("baseline.json"),
            vec![err_violation("naming/prefix", "/Game/Rock")],
        );

        let cfg = cfg_naming_error_with_baseline(std::path::Path::new("sub/baseline.json"));
        let result = handle_check_with_baseline(
            &dir,
            &[],
            &[],
            false,
            &db_path,
            &cfg,
            &FormatKind::Text,
            None,
            None,
        )
        .unwrap(); // would Err (not found) if resolved against CWD instead of project_dir
        assert_eq!(
            result, 0,
            "relative config baseline_path resolves against project_dir, not CWD"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- #276: auto-scan before checks ---

    #[test]
    fn auto_scan_should_create_db_when_skip_scan_is_false() {
        let (dir, db_path) = test_db_in_tempdir("check276_autoscan");
        // Empty content dir, no DB yet: the auto-scan must create it (scan ran).
        assert!(!db_path.exists());

        auto_scan(false, &dir, &db_path, &Default::default()).unwrap();

        assert!(
            db_path.exists(),
            "without --skip-scan, the pre-check scan runs and creates the DB"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn auto_scan_should_be_noop_when_skip_scan_is_true() {
        let (dir, db_path) = test_db_in_tempdir("check276_skipscan");
        assert!(!db_path.exists());

        auto_scan(true, &dir, &db_path, &Default::default()).unwrap();

        assert!(
            !db_path.exists(),
            "with --skip-scan, the scan is not invoked (no DB created)"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- #278: verbose / github-actions full output ---

    #[test]
    fn finding_lines_should_truncate_to_sample_limit_with_more_line_when_not_full() {
        let findings: Vec<String> = (0..10).map(|i| format!("/Game/A{i}")).collect();
        let lines = finding_lines(&findings, false, "dead-assets", std::path::Path::new("./P"));
        // 5 sample items + 1 truncation line
        assert_eq!(lines.len(), 6);
        assert!(
            lines.last().unwrap().contains("... and 5 more"),
            "non-full output must end with a truncation line"
        );
        assert_eq!(
            lines.iter().filter(|l| l.contains("/Game/A")).count(),
            5,
            "only CHECK_SAMPLE_LIMIT items are shown"
        );
    }

    #[test]
    fn finding_lines_should_show_all_items_without_more_line_when_full() {
        let findings: Vec<String> = (0..10).map(|i| format!("/Game/A{i}")).collect();
        let lines = finding_lines(&findings, true, "dead-assets", std::path::Path::new("./P"));
        assert_eq!(lines.len(), 10);
        assert!(
            lines.iter().all(|l| !l.contains("...")),
            "full output must not contain a truncation line"
        );
    }

    #[test]
    fn is_full_output_should_be_true_for_github_actions_regardless_of_verbose() {
        assert!(is_full_output(false, &FormatKind::GithubActions));
        assert!(is_full_output(true, &FormatKind::GithubActions));
        assert!(!is_full_output(false, &FormatKind::Text));
        assert!(is_full_output(true, &FormatKind::Text));
    }

    #[test]
    fn handle_check_sarif_should_emit_and_keep_blocking_exit_code() {
        let (dir, db_path) = test_db_in_tempdir("check292_sarif");
        upsert_misnamed_referenced_texture(&dir, &db_path);
        // naming severity = error → the lint check blocks → exit 1, same as other formats.
        let cfg = cfg_with_naming_severity(CheckSeverity::Error);
        let result =
            handle_check(&dir, &[], &[], false, &db_path, &cfg, &FormatKind::Sarif).unwrap();
        assert_eq!(result, 1, "sarif mode keeps the format-agnostic exit code");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_check_findings_should_map_severity_and_resolve_uri() {
        use std::path::PathBuf;
        use uasset_lens_shared::{AssetPath, AssetType};
        let assets = vec![uasset_lens_asset_db::AssetRecord {
            id: 0,
            asset_path: AssetPath::new("/Game/Rock").unwrap(),
            file_path: PathBuf::from("/proj/Content/Rock.uasset"),
            asset_type: AssetType::Texture2D,
            file_size: 0,
            last_modified: 0,
        }];
        let violations = vec![
            Violation {
                severity: ViolationSeverity::Error,
                rule: "naming/prefix".to_owned(),
                asset_path: "/Game/Rock".to_owned(),
                message: "m".to_owned(),
            },
            Violation {
                severity: ViolationSeverity::Warn,
                rule: "dead-assets".to_owned(),
                asset_path: "/Game/Rock".to_owned(),
                message: "m".to_owned(),
            },
            Violation {
                severity: ViolationSeverity::Warn,
                rule: "duplicate-assets".to_owned(),
                asset_path: "Rock".to_owned(), // group name, not a real asset path
                message: "m".to_owned(),
            },
        ];
        let f = build_check_findings(&violations, &assets, std::path::Path::new("/proj"));
        assert!(matches!(f[0].level, crate::sarif::SarifLevel::Error));
        assert_eq!(f[0].uri.as_deref(), Some("Content/Rock.uasset"));
        // A graph-only finding (dead-assets) still resolves its project-relative file uri.
        assert!(matches!(f[1].level, crate::sarif::SarifLevel::Warning));
        assert_eq!(f[1].uri.as_deref(), Some("Content/Rock.uasset"));
        // A duplicate group name has no backing file → no location.
        assert!(f[2].uri.is_none());
    }
}
