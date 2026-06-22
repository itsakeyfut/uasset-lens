use clap::Parser;

use crate::cli::{Cli, Commands, ConfigCommand, FormatKind};
use crate::commands;
use crate::resolve_db_path;
use crate::util::sarif_not_supported;

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
        | Commands::Doctor { .. }
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
                        quiet: cli.quiet,
                        suppress_report: false,
                        no_color: cli.no_color,
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
                asset_type,
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
            sort,
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
                *sort,
                refs.as_deref(),
                deps.as_deref(),
                &db_path,
                &cfg,
                &cli.format,
            )
        }
        Commands::Blueprint { project_dir } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::blueprint::handle_blueprint(project_dir, &db_path, &cli.format, cli.quiet)
        }
        Commands::Stats { project_dir, top } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::stats::handle_stats(project_dir, *top, &db_path, &cfg, &cli.format)
        }
        Commands::Budget { project_dir } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::budget::handle_budget(project_dir, &db_path, &cfg, &cli.format, cli.quiet)
        }
        Commands::Duplicates { project_dir } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::duplicates::handle_duplicates(project_dir, &db_path, &cli.format)
        }
        Commands::Lint { project_dir } => {
            let db_path = resolve_db_path(project_dir, cli.db.as_deref());
            commands::lint::handle_lint(project_dir, &db_path, &cfg, &cli.format, cli.quiet)
        }
        Commands::Check {
            project_dir,
            only,
            skip,
            verbose,
            save_baseline,
            diff_from,
            skip_scan,
            fail_on,
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
                *fail_on,
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
        Commands::Doctor { project_dir } => commands::doctor::handle_doctor(
            project_dir.as_deref(),
            cli.db.as_deref(),
            cli.config.as_deref(),
            &cli.format,
        ),
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn dispatch_should_reject_sarif_for_unsupported_command() {
        let cli = Cli {
            format: FormatKind::Sarif,
            db: None,
            yes: false,
            config: None,
            quiet: false,
            no_color: false,
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
