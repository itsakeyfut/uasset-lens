mod commands;
pub mod config;
mod ignore;
pub(crate) mod lint_builder;
mod paths;
mod sarif;

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
    /// SARIF 2.1.0 — supported by check / lint / budget only.
    #[value(name = "sarif")]
    Sarif,
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
    #[arg(
        long,
        global = true,
        help = "Path to config file (default: <project_dir>/.uasset-lens.toml)"
    )]
    pub config: Option<PathBuf>,
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
        /// Exclude paths matching this pattern, in addition to config (repeatable; prefix or glob)
        #[arg(long = "exclude")]
        exclude: Vec<String>,
        /// Preview scanned vs. excluded paths without writing to the database
        #[arg(long)]
        dry_run: bool,
        /// Suppress the animated progress bar
        #[arg(long)]
        no_progress: bool,
    },
    /// Show the dependency graph summary and detect circular dependencies
    Graph {
        project_dir: PathBuf,
        /// Show only circular dependencies
        #[arg(long)]
        cycles_only: bool,
        /// Show all nodes in long cycles instead of first 2 and last
        #[arg(long)]
        full_cycles: bool,
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
        /// Include sub-object types excluded by default (MetaData, BillboardComponent, etc.)
        #[arg(long)]
        include_all_types: bool,
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
        /// Number of asset types, folders, and largest assets to show
        /// (default: 10 types, 5 folders, 10 assets); 0 = show all
        #[arg(long)]
        top: Option<usize>,
    },
    /// Report assets exceeding configured per-type size budgets
    Budget { project_dir: PathBuf },
    /// List same-name and texture duplicate asset groups
    Duplicates { project_dir: PathBuf },
    /// Run all lint rules and report violations (exit 1 if any found).
    /// Size budget rules (budget/*) are controlled by [budget] in .uasset-lens.toml.
    Lint { project_dir: PathBuf },
    /// Run all (or selected) health checks; exits 1 if any check finds problems
    #[command(name = "check")]
    Check {
        project_dir: PathBuf,
        /// Run only these checks (comma-separated: dead-assets,cycles,redirectors,lint,budget,duplicates)
        #[arg(long, value_delimiter = ',')]
        only: Vec<String>,
        /// Skip these checks (comma-separated: dead-assets,cycles,redirectors,lint,budget,duplicates)
        #[arg(long, value_delimiter = ',', conflicts_with = "only")]
        skip: Vec<String>,
        /// Show all findings instead of the first 5 per category
        #[arg(long)]
        verbose: bool,
        /// Save current violations as a baseline JSON (default: .uasset-lens/baselines/check-baseline.json)
        #[arg(long, num_args(0..=1), default_missing_value = ".uasset-lens/baselines/check-baseline.json")]
        save_baseline: Option<PathBuf>,
        /// Compare against a baseline; exit 1 only on new error regressions
        #[arg(long)]
        diff_from: Option<PathBuf>,
        /// Skip the pre-check mtime delta scan (use the existing DB as-is)
        #[arg(long)]
        skip_scan: bool,
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
    /// Generate a starter .uasset-lens.toml from a project-scale preset
    Init {
        project_dir: PathBuf,
        /// Preset to apply without prompting (indie, mid, aaa)
        #[arg(long, value_enum)]
        preset: Option<commands::init::Preset>,
        /// Overwrite an existing .uasset-lens.toml
        #[arg(long)]
        force: bool,
    },
    /// Inspect and validate the project configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Validate .uasset-lens.toml syntax, field types, and value constraints
    Validate {
        /// Project directory containing .uasset-lens.toml (default: current directory)
        project_dir: Option<PathBuf>,
    },
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
    // SARIF is only meaningful for the violation-producing commands; reject it elsewhere
    // up front so the command does no work before failing.
    if matches!(cli.format, FormatKind::Sarif)
        && !matches!(
            cli.command,
            Commands::Check { .. } | Commands::Lint { .. } | Commands::Budget { .. }
        )
    {
        return Err(sarif_not_supported());
    }

    let project_dir_for_cfg = match &cli.command {
        Commands::Scan { project_dir, .. }
        | Commands::Graph { project_dir, .. }
        | Commands::DeadAssets { project_dir, .. }
        | Commands::Deps { project_dir, .. }
        | Commands::Impact { project_dir, .. }
        | Commands::Redirectors { project_dir }
        | Commands::Find { project_dir, .. }
        | Commands::Blueprint { project_dir }
        | Commands::Stats { project_dir, .. }
        | Commands::Budget { project_dir }
        | Commands::Duplicates { project_dir }
        | Commands::Lint { project_dir }
        | Commands::Check { project_dir, .. }
        | Commands::Clean { project_dir, .. }
        | Commands::Watch { project_dir } => Some(project_dir.as_path()),
        // `init` creates the config; `config validate` reads it directly. Neither loads one here.
        Commands::Path { .. }
        | Commands::Completions { .. }
        | Commands::Init { .. }
        | Commands::Config { .. } => None,
    };

    let mut cfg = if let Some(pd) = project_dir_for_cfg {
        crate::config::resolve_config(pd, cli.config.as_deref())?
    } else {
        crate::config::ConfigFile::default()
    };

    match &cli.command {
        Commands::Scan {
            project_dir,
            full_scan,
            diff,
            save_baseline,
            diff_from,
            exclude,
            dry_run,
            no_progress,
        } => {
            // `--exclude` patterns are additive one-off exclusions on top of config.
            cfg.scan.exclude_paths.extend(exclude.iter().cloned());
            if *dry_run {
                commands::scan::handle_scan_dry_run(project_dir, &cfg, &cli.format)
            } else {
                let db_path = resolve_db_path(project_dir, cli.db.as_deref());
                commands::scan::handle_scan(
                    project_dir,
                    &db_path,
                    &cli.format,
                    &cfg,
                    &commands::scan::ScanOptions {
                        full_scan: *full_scan,
                        diff: *diff,
                        yes: cli.yes,
                        save_baseline: save_baseline.as_deref(),
                        diff_from: diff_from.as_deref(),
                        quiet: false,
                        no_progress: *no_progress,
                    },
                )
            }
        }
        Commands::Graph {
            project_dir,
            cycles_only,
            full_cycles,
        } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::graph::handle_graph(
                project_dir,
                *cycles_only,
                *full_cycles,
                &db_path,
                &cfg,
                &cli.format,
            )
        }
        Commands::DeadAssets {
            project_dir,
            asset_type,
            sort_by_size,
            min_size,
            exclude_patterns,
            group,
            include_all_types,
        } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::dead_assets::handle_dead_assets(
                project_dir,
                asset_type.as_deref(),
                *sort_by_size,
                *min_size,
                exclude_patterns,
                group.as_ref(),
                *include_all_types,
                &db_path,
                &cfg,
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
                &cfg,
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
                &cfg,
                *tree,
                &cli.format,
            )
        }
        Commands::Redirectors { project_dir } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::redirectors::handle_redirectors(project_dir, &db_path, &cfg, &cli.format)
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
                &cfg,
                &cli.format,
            )
        }
        Commands::Blueprint { project_dir } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::blueprint::handle_blueprint(project_dir, &db_path, &cli.format)
        }
        Commands::Stats { project_dir, top } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::stats::handle_stats(project_dir, *top, &db_path, &cfg, &cli.format)
        }
        Commands::Budget { project_dir } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::budget::handle_budget(project_dir, &db_path, &cfg, &cli.format)
        }
        Commands::Duplicates { project_dir } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::duplicates::handle_duplicates(project_dir, &db_path, &cli.format)
        }
        Commands::Lint { project_dir } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::lint::handle_lint(project_dir, &db_path, &cfg, &cli.format)
        }
        Commands::Check {
            project_dir,
            only,
            skip,
            verbose,
            save_baseline,
            diff_from,
            skip_scan,
        } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::check::auto_scan(*skip_scan, project_dir, &db_path, &cfg)?;
            commands::check::handle_check_with_baseline(
                project_dir,
                only,
                skip,
                *verbose,
                &db_path,
                &cfg,
                &cli.format,
                save_baseline.as_deref(),
                diff_from.as_deref(),
            )
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
                &cfg,
                &cli.format,
            )
        }
        Commands::Watch { project_dir } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::watch::handle_watch(project_dir, &db_path, &cfg)
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
        Commands::Init {
            project_dir,
            preset,
            force,
        } => commands::init::handle_init(project_dir, *preset, *force, cli.yes, &cli.format),
        Commands::Config { command } => match command {
            ConfigCommand::Validate { project_dir } => {
                commands::config_validate::handle_config_validate(
                    project_dir.as_deref(),
                    cli.config.as_deref(),
                    &cli.format,
                )
            }
        },
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

/// Error returned when `--format sarif` is used on a command that does not produce violations.
pub(crate) fn sarif_not_supported() -> anyhow::Error {
    anyhow::anyhow!("--format sarif is only supported by the check, lint, and budget commands")
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

    #[test]
    fn dispatch_should_use_explicit_config_path_when_config_flag_is_set() {
        let dir = std::env::temp_dir().join(format!("uasset_lens_dispatch_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg_path = dir.join("custom.toml");
        std::fs::write(&cfg_path, "[lint.blueprint]\ndependency_depth_limit = 99\n").unwrap();

        let cfg =
            crate::config::resolve_config(std::path::Path::new("."), Some(&cfg_path)).unwrap();
        assert_eq!(cfg.lint.blueprint.dependency_depth_limit, Some(99));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dispatch_should_error_when_explicit_config_path_does_not_exist() {
        let absent = std::env::temp_dir().join(format!(
            "uasset_lens_absent_dispatch_{}.toml",
            std::process::id()
        ));
        let result = crate::config::resolve_config(std::path::Path::new("."), Some(&absent));
        assert!(result.is_err());
    }

    #[test]
    fn check_should_reject_only_and_skip_used_together() {
        use clap::Parser;
        let result = Cli::try_parse_from([
            "uasset-lens",
            "check",
            "./Project",
            "--only",
            "cycles",
            "--skip",
            "lint",
        ]);
        let err = result.expect_err("--only and --skip together must be a clap error");
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn dispatch_should_reject_sarif_for_unsupported_command() {
        let cli = Cli {
            format: FormatKind::Sarif,
            db: None,
            yes: false,
            config: None,
            command: Commands::Scan {
                project_dir: std::path::PathBuf::from("."),
                full_scan: false,
                diff: false,
                save_baseline: None,
                diff_from: None,
                exclude: vec![],
                dry_run: false,
                no_progress: false,
            },
        };
        let result = dispatch(&cli);
        assert!(
            result.is_err(),
            "--format sarif on scan must fail fast with an error"
        );
    }
}
