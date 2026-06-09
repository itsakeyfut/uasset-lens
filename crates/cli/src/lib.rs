mod commands;
pub mod config;
pub(crate) mod lint_builder;
mod paths;

use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};

pub(crate) use paths::{find_project_dir, resolve_asset_path};
pub use paths::{resolve_content_root, resolve_db_path};

#[derive(Debug, Clone, Default, PartialEq, ValueEnum)]
pub enum FormatKind {
    #[default]
    Text,
    Json,
    #[value(name = "github-actions")]
    GithubActions,
}

#[derive(Debug, Clone, PartialEq, ValueEnum)]
pub enum GroupMode {
    #[value(name = "type")]
    Type,
    Dir,
}

#[derive(Debug, Parser)]
#[command(name = "uasset-lens", about = "Unreal Engine 5 asset static analyzer")]
pub struct Cli {
    #[arg(long, value_enum, default_value_t = FormatKind::Text, global = true)]
    pub format: FormatKind,
    /// Override the database file location
    #[arg(long, global = true)]
    pub db: Option<PathBuf>,
    /// Skip confirmation prompts (for CI)
    #[arg(short = 'y', long, global = true)]
    pub yes: bool,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Scan a project directory and index all assets into the database
    Scan {
        project_dir: PathBuf,
        /// Re-scan all files regardless of modification time
        #[arg(long)]
        full_scan: bool,
        /// Show a diff of changes compared to the previous scan
        #[arg(long, conflicts_with = "diff_from")]
        diff: bool,
        /// Save the scan result as a named baseline for later --diff-from comparisons
        #[arg(long)]
        save_baseline: Option<String>,
        /// Diff against a named baseline instead of the previous scan (implies --diff)
        #[arg(long)]
        diff_from: Option<String>,
    },
    /// Show the dependency graph summary and detect circular dependencies
    Graph {
        project_dir: PathBuf,
        /// Show only circular dependencies
        #[arg(long)]
        cycles_only: bool,
    },
    /// List assets that are not referenced by any other asset
    #[command(name = "dead-assets")]
    DeadAssets {
        project_dir: PathBuf,
        /// Filter by asset type (e.g. Texture2D, Blueprint)
        #[arg(long = "type")]
        asset_type: Option<String>,
        /// Sort results by file size, largest first
        #[arg(long)]
        sort_by_size: bool,
        /// Exclude assets smaller than this many bytes
        #[arg(long)]
        min_size: Option<u64>,
        /// Exclude assets whose path contains this substring (repeatable)
        #[arg(long = "exclude")]
        exclude_patterns: Vec<String>,
        /// Aggregate results by asset type or top-level directory
        #[arg(long)]
        group: Option<GroupMode>,
    },
    /// Show the forward dependency tree of an asset
    Deps {
        project_dir: PathBuf,
        asset_path: PathBuf,
        /// Maximum recursion depth (default: unlimited)
        #[arg(long)]
        depth: Option<u32>,
        /// Print only the summary line, not the full tree
        #[arg(long)]
        size_only: bool,
    },
    /// Show which assets would break if the target asset were deleted or renamed
    Impact {
        project_dir: PathBuf,
        asset_path: PathBuf,
        /// Show the full propagation tree instead of flat lists
        #[arg(long)]
        tree: bool,
    },
    /// List all ObjectRedirector assets in the project
    Redirectors { project_dir: PathBuf },
    /// Search and filter assets by type, size, or path pattern
    Find {
        project_dir: PathBuf,
        /// Filter by asset type (e.g. Texture2D, Blueprint)
        #[arg(long = "type")]
        asset_type: Option<String>,
        /// Minimum file size in bytes
        #[arg(long)]
        larger_than: Option<u64>,
        /// Maximum file size in bytes
        #[arg(long)]
        smaller_than: Option<u64>,
        /// Only show unreferenced assets
        #[arg(long)]
        unreferenced: bool,
        /// Filter by glob path pattern (e.g. "**/Characters/**")
        #[arg(long)]
        path: Option<String>,
        /// Sort results by file size, largest first
        #[arg(long)]
        sort_by_size: bool,
        /// Show only assets that reference this game path (direct + transitive)
        #[arg(long)]
        refs: Option<String>,
        /// Show only assets that this game path directly depends on
        #[arg(long)]
        deps: Option<String>,
    },
    /// Show a complexity ranking of Blueprint assets
    Blueprint { project_dir: PathBuf },
    /// Show a size and composition overview of the project
    Stats {
        project_dir: PathBuf,
        /// Number of folders and largest assets to show (default: 5 folders, 10 assets)
        #[arg(long)]
        top: Option<usize>,
    },
    /// Report assets exceeding configured per-type size budgets
    Budget { project_dir: PathBuf },
    /// List same-name and texture duplicate asset groups
    Duplicates { project_dir: PathBuf },
    /// Run all lint rules and report violations (exit 1 if any found)
    Lint { project_dir: PathBuf },
    /// Run all (or selected) health checks; exits 1 if any check finds problems
    #[command(name = "check")]
    Check {
        project_dir: PathBuf,
        /// Run only these checks (comma-separated: dead-assets,cycles,redirectors,lint,budget,duplicates)
        #[arg(long, value_delimiter = ',')]
        only: Vec<String>,
        /// Skip these checks (comma-separated: dead-assets,cycles,redirectors,lint,budget,duplicates)
        #[arg(long, value_delimiter = ',')]
        skip: Vec<String>,
    },
    /// Delete confirmed dead assets from disk
    #[command(name = "clean")]
    Clean {
        project_dir: PathBuf,
        /// List deletion targets without deleting and exit 0
        #[arg(long)]
        dry_run: bool,
        /// Exclude assets smaller than this many bytes
        #[arg(long)]
        min_size: Option<u64>,
        /// Exclude assets whose path contains this substring (repeatable)
        #[arg(long = "exclude")]
        exclude_patterns: Vec<String>,
        /// Filter by glob path pattern (e.g. "**/Characters/**")
        #[arg(long)]
        path: Option<String>,
    },
    /// Watch the project directory and print new problems as files change
    Watch { project_dir: PathBuf },
    /// Convert between filesystem paths and UE game paths
    #[command(name = "path")]
    Path {
        /// The path to convert (filesystem path or /Game/... game path)
        input: String,
        /// Convert a game path to a filesystem path
        #[arg(long)]
        to_file: bool,
        /// Content root directory (auto-detected if not provided)
        #[arg(long)]
        content_root: Option<PathBuf>,
    },
    /// Generate shell completion scripts
    #[command(name = "completions")]
    Completions {
        /// Shell to generate completions for (bash, zsh, fish, powershell, elvish)
        shell: String,
    },
}

/// Opens an existing database, translating `DbError::NotFound` into a user-friendly CLI message.
pub(crate) fn open_db(db_path: &Path) -> anyhow::Result<asset_db::AssetDb> {
    asset_db::AssetDb::open_existing(db_path).map_err(|e| match e {
        asset_db::DbError::NotFound(_) => {
            anyhow::anyhow!("no scan data found.\nRun 'uasset-lens scan <project_dir>' first.")
        }
        other => anyhow::Error::from(other),
    })
}

pub(crate) fn load_graph(
    db: &asset_db::AssetDb,
    external_roots: &[String],
) -> anyhow::Result<dependency_graph::DependencyGraph> {
    let records = db
        .all_assets()
        .context("Failed to read assets from database")?;
    let nodes: Vec<dependency_graph::AssetNode> = records
        .iter()
        .map(|r| dependency_graph::AssetNode {
            path: r.asset_path.clone(),       // clone required: AssetPath is not Copy
            asset_type: r.asset_type.clone(), // clone required: AssetType is not Copy
        })
        .collect();
    let edges = db
        .all_edges()
        .context("Failed to read dependency edges from database")?;
    Ok(dependency_graph::DependencyGraph::build(
        nodes,
        edges,
        external_roots,
    ))
}

pub fn run() -> i32 {
    run_with(Cli::parse())
}

/// Runs the CLI with a pre-parsed `Cli` instance.
/// Callers that intercept specific commands (e.g., `completions`) before dispatch use this.
pub fn run_with(cli: Cli) -> i32 {
    match dispatch(&cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {e:#}");
            2
        }
    }
}

fn dispatch(cli: &Cli) -> anyhow::Result<i32> {
    match &cli.command {
        Commands::Scan {
            project_dir,
            full_scan,
            diff,
            save_baseline,
            diff_from,
        } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::scan::handle_scan(
                project_dir,
                &db_path,
                &cli.format,
                &commands::scan::ScanOptions {
                    full_scan: *full_scan,
                    diff: *diff,
                    yes: cli.yes,
                    save_baseline: save_baseline.as_deref(),
                    diff_from: diff_from.as_deref(),
                },
            )
        }
        Commands::Graph {
            project_dir,
            cycles_only,
        } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::graph::handle_graph(project_dir, *cycles_only, &db_path, &cli.format)
        }
        Commands::DeadAssets {
            project_dir,
            asset_type,
            sort_by_size,
            min_size,
            exclude_patterns,
            group,
        } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::dead_assets::handle_dead_assets(
                project_dir,
                asset_type.as_deref(),
                *sort_by_size,
                *min_size,
                exclude_patterns,
                group.as_ref(),
                &db_path,
                &cli.format,
            )
        }
        Commands::Deps {
            project_dir,
            asset_path,
            depth,
            size_only,
        } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::deps::handle_deps(
                project_dir,
                asset_path,
                &db_path,
                *depth,
                *size_only,
                &cli.format,
            )
        }
        Commands::Impact {
            project_dir,
            asset_path,
            tree,
        } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::impact::handle_impact(
                project_dir,
                asset_path,
                &db_path,
                *tree,
                &cli.format,
            )
        }
        Commands::Redirectors { project_dir } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::redirectors::handle_redirectors(project_dir, &db_path, &cli.format)
        }
        Commands::Find {
            project_dir,
            asset_type,
            larger_than,
            smaller_than,
            unreferenced,
            path,
            sort_by_size,
            refs,
            deps,
        } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::find::handle_find(
                project_dir,
                asset_type.as_deref(),
                *larger_than,
                *smaller_than,
                *unreferenced,
                path.as_deref(),
                *sort_by_size,
                refs.as_deref(),
                deps.as_deref(),
                &db_path,
                &cli.format,
            )
        }
        Commands::Blueprint { project_dir } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::blueprint::handle_blueprint(project_dir, &db_path, &cli.format)
        }
        Commands::Stats { project_dir, top } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::stats::handle_stats(project_dir, *top, &db_path, &cli.format)
        }
        Commands::Budget { project_dir } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::budget::handle_budget(project_dir, &db_path, &cli.format)
        }
        Commands::Duplicates { project_dir } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::duplicates::handle_duplicates(project_dir, &db_path, &cli.format)
        }
        Commands::Lint { project_dir } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::lint::handle_lint(project_dir, &db_path, &cli.format)
        }
        Commands::Check {
            project_dir,
            only,
            skip,
        } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::check::handle_check(project_dir, only, skip, &db_path, &cli.format)
        }
        Commands::Clean {
            project_dir,
            dry_run,
            min_size,
            exclude_patterns,
            path,
        } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::clean::handle_clean(
                project_dir,
                cli.yes,
                *dry_run,
                *min_size,
                exclude_patterns,
                path.as_deref(),
                &db_path,
                &cli.format,
            )
        }
        Commands::Watch { project_dir } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::watch::handle_watch(project_dir, &db_path)
        }
        Commands::Path {
            input,
            to_file,
            content_root,
        } => commands::path_conv::handle_path_conv(
            input,
            *to_file,
            content_root.as_deref(),
            &cli.format,
        ),
        // Intercepted before dispatch in apps/uasset-lens-cli/src/main.rs
        Commands::Completions { .. } => {
            unreachable!("completions is handled at the binary entry point before dispatch")
        }
    }
}

pub(crate) fn maybe_hint_github_actions(format: &FormatKind) {
    if !matches!(format, FormatKind::GithubActions)
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
