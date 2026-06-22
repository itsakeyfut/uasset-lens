use std::path::Path;

use anyhow::Context;

use crate::cli::FormatKind;

/// Whether ANSI color should be emitted: text format, not disabled by `--no-color` or a non-empty
/// `NO_COLOR` env var (https://no-color.org), and stdout is a TTY.
pub(crate) fn use_color(format: &FormatKind, no_color: bool) -> bool {
    use std::io::IsTerminal;
    matches!(format, FormatKind::Text)
        && !no_color
        && !env_disables_color(std::env::var_os("NO_COLOR").as_deref())
        && std::io::stdout().is_terminal()
}

fn env_disables_color(value: Option<&std::ffi::OsStr>) -> bool {
    value.is_some_and(|v| !v.is_empty())
}

/// Opens an existing database, translating `DbError::NotFound` into a user-friendly CLI message.
pub(crate) fn open_db(db_path: &Path) -> anyhow::Result<uasset_lens_asset_db::AssetDb> {
    uasset_lens_asset_db::AssetDb::open_existing(db_path).map_err(|e| match e {
        uasset_lens_asset_db::DbError::NotFound(_) => {
            anyhow::anyhow!("no scan data found.\nRun 'uasset-lens scan <project_dir>' first.")
        }
        other => anyhow::Error::from(other),
    })
}

pub(crate) fn load_graph(
    db: &uasset_lens_asset_db::AssetDb,
    external_roots: &[String],
) -> anyhow::Result<uasset_lens_dependency_graph::DependencyGraph> {
    let records = db
        .all_assets()
        .context("Failed to read assets from database")?;
    let nodes: Vec<uasset_lens_dependency_graph::AssetNode> = records
        .iter()
        .map(|r| uasset_lens_dependency_graph::AssetNode {
            path: r.asset_path.clone(),       // clone required: AssetPath is not Copy
            asset_type: r.asset_type.clone(), // clone required: AssetType is not Copy
        })
        .collect();
    let edges = db
        .all_edges()
        .context("Failed to read dependency edges from database")?;
    Ok(uasset_lens_dependency_graph::DependencyGraph::build(
        nodes,
        edges,
        external_roots,
    ))
}

pub(crate) fn maybe_hint_github_actions(format: &FormatKind, quiet: bool) {
    if !quiet
        && !matches!(format, FormatKind::GithubActions)
        && (std::env::var("GITHUB_ACTIONS").is_ok()
            || std::env::var("ACTIONS_RUNNER_ENVIRONMENT").is_ok())
    {
        eprintln!(
            "Hint: running inside GitHub Actions — \
             add '--format github-actions' to get inline PR annotations."
        );
    }
}

pub(crate) fn rel_path_for_annotation(file_path: &Path, project_dir: &Path) -> String {
    file_path
        .strip_prefix(project_dir)
        .unwrap_or(file_path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(crate) fn format_gh_annotation(level: &str, rel: &str, title: &str, message: &str) -> String {
    format!("::{level} file={rel},title={title}::{message}")
}

pub(crate) fn format_size(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    const KIB: u64 = 1024;
    if bytes >= MIB {
        format!("{:.1} MB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KB", bytes as f64 / KIB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// The canonical budget rule id, uniform across all asset types and commands
/// (lint / budget / check / SARIF). Keeping a single generator avoids divergence.
pub(crate) fn budget_rule_id(asset_type: &uasset_lens_shared::AssetType) -> String {
    format!("budget/{}", asset_type.to_string().to_lowercase())
}

pub(crate) fn path_depth_prefix(path: &str) -> &str {
    let mut slash_count = 0;
    let mut last_cut = path.len();
    for (i, c) in path.char_indices() {
        if c == '/' {
            slash_count += 1;
            if slash_count == 4 {
                return &path[..i + 1];
            }
            if slash_count == 3 {
                last_cut = i + 1;
            }
        }
    }
    // Trailing-slash consistency: 3-slash paths get the same treatment as 4+ slash paths.
    if slash_count >= 3 {
        &path[..last_cut]
    } else {
        path
    }
}

pub(crate) fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

pub(crate) fn digit_count(n: usize) -> usize {
    if n == 0 { 1 } else { n.ilog10() as usize + 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn env_disables_color_should_follow_no_color_spec() {
        // https://no-color.org: any non-empty value disables; empty or unset does not.
        assert!(env_disables_color(Some(OsStr::new("1"))));
        assert!(env_disables_color(Some(OsStr::new("anything"))));
        assert!(!env_disables_color(Some(OsStr::new(""))));
        assert!(!env_disables_color(None));
    }

    #[test]
    fn use_color_should_be_false_when_no_color_flag_set() {
        assert!(!use_color(&FormatKind::Text, true));
    }

    #[test]
    fn use_color_should_be_false_for_non_text_format() {
        // JSON / SARIF must never contain ANSI codes, regardless of TTY or --no-color.
        assert!(!use_color(&FormatKind::Json, false));
        assert!(!use_color(&FormatKind::Sarif, false));
    }

    #[test]
    fn rel_path_for_annotation_should_strip_project_prefix() {
        let project = std::path::Path::new("/proj");
        let file = std::path::Path::new("/proj/Content/T_Rock.uasset");
        assert_eq!(
            rel_path_for_annotation(file, project),
            "Content/T_Rock.uasset"
        );
    }

    #[test]
    fn rel_path_for_annotation_should_use_full_path_when_prefix_not_matched() {
        let project = std::path::Path::new("/proj");
        let file = std::path::Path::new("/other/T_Rock.uasset");
        let result = rel_path_for_annotation(file, project);
        assert_eq!(result, "/other/T_Rock.uasset");
    }

    #[test]
    fn format_gh_annotation_should_produce_correct_workflow_command() {
        let s = format_gh_annotation(
            "error",
            "Content/T_Rock.uasset",
            "BudgetOverrun",
            "Texture2D 2.0 MB exceeds limit 1.0 MB",
        );
        assert_eq!(
            s,
            "::error file=Content/T_Rock.uasset,title=BudgetOverrun::Texture2D 2.0 MB exceeds limit 1.0 MB"
        );
    }

    #[test]
    fn format_size_should_format_bytes_as_human_readable() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(2048), "2.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(2 * 1024 * 1024), "2.0 MB");
    }

    #[test]
    fn path_depth_prefix_should_return_path_up_to_third_segment() {
        assert_eq!(
            path_depth_prefix("/Game/Assets/Enemies/BP_Hero"),
            "/Game/Assets/Enemies/"
        );
        assert_eq!(
            path_depth_prefix("/Game/ThirdPerson/Blueprints/BP_Char"),
            "/Game/ThirdPerson/Blueprints/"
        );
        assert_eq!(
            path_depth_prefix("/Game/Characters/BP_Hero"),
            "/Game/Characters/"
        );
    }

    #[test]
    fn path_depth_prefix_should_return_full_path_when_fewer_than_three_segments() {
        assert_eq!(path_depth_prefix("/Game/Foo"), "/Game/Foo");
        assert_eq!(path_depth_prefix("/Game"), "/Game");
    }

    #[test]
    fn format_number_should_add_comma_separator_for_thousands() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1234567), "1,234,567");
    }

    #[test]
    fn digit_count_should_return_correct_digit_count() {
        assert_eq!(digit_count(0), 1);
        assert_eq!(digit_count(9), 1);
        assert_eq!(digit_count(10), 2);
        assert_eq!(digit_count(999), 3);
        assert_eq!(digit_count(1000), 4);
    }
}
