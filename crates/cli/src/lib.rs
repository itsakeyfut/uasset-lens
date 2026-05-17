mod commands;
pub mod config;
pub(crate) mod lint_builder;
mod paths;

use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};

pub use paths::{resolve_content_root, resolve_db_path};

#[derive(Debug, Clone, Default, PartialEq, ValueEnum)]
pub enum FormatKind {
    #[default]
    Text,
    Json,
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
        asset_path: PathBuf,
        /// Maximum recursion depth (default: unlimited)
        #[arg(long)]
        depth: Option<u32>,
        /// Print only the summary line, not the full tree
        #[arg(long)]
        size_only: bool,
    },
    /// Show which assets would break if the target asset were deleted or renamed
    Impact { asset_path: PathBuf },
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
    Ok(dependency_graph::DependencyGraph::build(nodes, edges))
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
        } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::scan::handle_scan(project_dir, *full_scan, &db_path, &cli.format, cli.yes)
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
            asset_path,
            depth,
            size_only,
        } => commands::deps::handle_deps(
            asset_path,
            cli.db.as_deref(),
            *depth,
            *size_only,
            &cli.format,
        ),
        Commands::Impact { asset_path } => {
            commands::impact::handle_impact(asset_path, cli.db.as_deref(), &cli.format)
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
        } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::find::handle_find(
                project_dir,
                asset_type.as_deref(),
                *larger_than,
                *smaller_than,
                *unreferenced,
                path.as_deref(),
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
