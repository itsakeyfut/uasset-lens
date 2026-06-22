use std::collections::HashSet;
use std::path::Path;

use anyhow::Context;

use crate::config::CheckSeverity;

/// Severity as serialized in the violation baseline. Only `Error` participates in
/// regression detection; `Warn` is recorded but never gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum ViolationSeverity {
    Error,
    Warn,
}

/// A single check finding in structured form, used for the baseline JSON.
/// `file` is intentionally omitted to keep comparisons path-stable across machines.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct Violation {
    pub(super) severity: ViolationSeverity,
    pub(super) rule: String,
    pub(super) asset_path: String,
    pub(super) message: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct BaselineSummary {
    pub(super) errors: usize,
    pub(super) warnings: usize,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct BaselineDoc {
    pub(super) version: u32,
    pub(super) git_commit: String,
    pub(super) summary: BaselineSummary,
    pub(super) violations: Vec<Violation>,
}

pub(super) fn to_violation_severity(s: CheckSeverity) -> ViolationSeverity {
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
pub(super) fn compute_regressions(baseline: &[Violation], current: &[Violation]) -> Vec<Violation> {
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

pub(super) fn load_baseline(path: &Path) -> anyhow::Result<BaselineDoc> {
    let s = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read baseline file: {}", path.display()))?;
    serde_json::from_str(&s).with_context(|| format!("invalid baseline JSON: {}", path.display()))
}

pub(super) fn save_baseline_file(
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
