mod cli;
mod commands;
pub mod config;
mod dispatch;
mod ignore;
pub(crate) mod lint_builder;
mod paths;
mod sarif;
mod time;
mod util;

pub use cli::*;
pub use dispatch::{run, run_with};
pub(crate) use paths::{find_project_dir, resolve_asset_path};
pub use paths::{resolve_content_root, resolve_db_path};
pub(crate) use util::{
    budget_rule_id, digit_count, format_gh_annotation, format_number, format_size, load_graph,
    maybe_hint_github_actions, open_db, path_depth_prefix, rel_path_for_annotation,
    sarif_not_supported, use_color,
};
