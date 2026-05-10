mod commands;
pub(crate) mod config;
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
}

pub(crate) fn load_graph(db_path: &Path) -> anyhow::Result<dependency_graph::DependencyGraph> {
    if !db_path.exists() {
        anyhow::bail!("no scan data found.\nRun 'uasset-lens scan <project_dir>' first.");
    }
    let db = asset_db::AssetDb::open(db_path).context("Failed to open database")?;
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
    let cli = Cli::parse();
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
        } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::dead_assets::handle_dead_assets(
                project_dir,
                asset_type.as_deref(),
                &db_path,
                &cli.format,
            )
        }
        Commands::Impact { asset_path } => {
            commands::impact::handle_impact(asset_path, cli.db.as_deref(), &cli.format)
        }
        Commands::Redirectors { project_dir } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::redirectors::handle_redirectors(project_dir, &db_path, &cli.format)
        }
        Commands::Find { .. } => todo!(),
    }
}
