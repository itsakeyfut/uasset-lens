use std::path::Path;

use crate::FormatKind;
use crate::config::CheckSeverity;

use super::CHECK_SAMPLE_LIMIT;
use super::baseline::{Violation, ViolationSeverity};

#[derive(serde::Serialize)]
pub(super) struct CheckResultJson {
    pub(super) name: String,
    pub(super) severity: &'static str,
    pub(super) passed: bool,
    pub(super) findings: Vec<String>,
}

pub(super) fn severity_label(severity: CheckSeverity) -> &'static str {
    match severity {
        CheckSeverity::Error => "error",
        CheckSeverity::Warn => "warn",
        CheckSeverity::Off => "off",
    }
}

pub(super) fn fail_on_label(fail_on: crate::FailOn) -> &'static str {
    match fail_on {
        crate::FailOn::Error => "error",
        crate::FailOn::Warn => "warn",
        crate::FailOn::Never => "never",
    }
}

#[derive(serde::Serialize)]
pub(super) struct CheckOutput {
    pub(super) passed: bool,
    pub(super) fail_on: &'static str,
    pub(super) checks: Vec<CheckResultJson>,
}

pub(super) fn check_display_name(name: &str) -> &str {
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

pub(super) fn check_full_command(name: &str) -> &str {
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
pub(super) fn build_check_findings(
    violations: &[Violation],
    assets: &[uasset_lens_asset_db::AssetRecord],
    project_dir: &Path,
) -> Vec<crate::sarif::SarifFinding> {
    let path_lookup = crate::path_lookup(assets);
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
pub(super) fn is_full_output(verbose: bool, format: &FormatKind) -> bool {
    verbose || matches!(format, FormatKind::GithubActions)
}

/// Renders one check's finding lines: up to `CHECK_SAMPLE_LIMIT` items, then a
/// "... and N more" line, unless `full_output` shows all items with no truncation line.
pub(super) fn finding_lines(
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
    use super::super::baseline::{Violation, ViolationSeverity};
    use super::*;

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
