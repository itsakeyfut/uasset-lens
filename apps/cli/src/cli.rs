use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::commands;

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

/// Severity threshold at which `check` exits 1. `error` is the default (only error-severity
/// findings fail); `warn` fails on any finding; `never` is informational (always exit 0).
#[derive(Debug, Clone, Copy, PartialEq, ValueEnum)]
pub enum FailOn {
    Error,
    Warn,
    Never,
}

/// Sort order for `find` results. `path` is the default (alphabetical by game path).
#[derive(Debug, Clone, Copy, PartialEq, ValueEnum)]
pub enum SortKey {
    Path,
    #[value(name = "size-desc")]
    SizeDesc,
    #[value(name = "size-asc")]
    SizeAsc,
    Type,
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
    /// Suppress progress and informational output on stderr (the result still goes to stdout)
    #[arg(long, global = true)]
    pub quiet: bool,
    /// Disable ANSI color output (also honored via the NO_COLOR environment variable)
    #[arg(long, global = true)]
    pub no_color: bool,
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
        /// Filter by asset type (e.g. Texture2D, Blueprint); repeatable, OR-combined
        #[arg(long = "type")]
        asset_type: Vec<String>,
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
        /// Sort order: path (default), size-desc, size-asc, or type
        #[arg(long, value_enum, default_value_t = SortKey::Path)]
        sort: SortKey,
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
        /// Severity at which to exit 1: error (default), warn (any finding), or never
        #[arg(long, value_enum, default_value_t = FailOn::Error)]
        fail_on: FailOn,
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
    /// Check installation health: DB, schema version, config, scan freshness, scanner compat
    Doctor {
        /// Project directory to diagnose (default: current directory)
        project_dir: Option<PathBuf>,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
