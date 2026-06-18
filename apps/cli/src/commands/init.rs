use std::io::IsTerminal;
use std::path::Path;

use anyhow::Context;

use crate::FormatKind;

/// Project-scale preset selecting which starter `.uasset-lens.toml` to write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Preset {
    Indie,
    Mid,
    Aaa,
}

impl Preset {
    fn name(self) -> &'static str {
        match self {
            Preset::Indie => "indie",
            Preset::Mid => "mid",
            Preset::Aaa => "aaa",
        }
    }

    fn template(self) -> &'static str {
        match self {
            Preset::Indie => include_str!("init/templates/indie.toml"),
            Preset::Mid => include_str!("init/templates/mid.toml"),
            Preset::Aaa => include_str!("init/templates/aaa.toml"),
        }
    }
}

fn parse_preset(input: &str) -> Option<Preset> {
    match input.trim().to_lowercase().as_str() {
        "indie" => Some(Preset::Indie),
        "mid" => Some(Preset::Mid),
        "aaa" => Some(Preset::Aaa),
        _ => None,
    }
}

/// Writes a starter `.uasset-lens.toml` for the chosen preset. Interactive when no preset is
/// given and stdin is a TTY; otherwise defaults to `indie`. Prompts go to stderr so `--format
/// json` keeps stdout to a single value.
pub(crate) fn handle_init(
    project_dir: &Path,
    preset: Option<Preset>,
    force: bool,
    yes: bool,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    let json = matches!(format, FormatKind::Json);
    let config_path = project_dir.join(".uasset-lens.toml");

    if config_path.exists() && !force {
        if json {
            print_json(&serde_json::json!({
                "written": false,
                "path": ".uasset-lens.toml",
                "error": "config_exists",
            }))?;
        } else {
            eprintln!("error: .uasset-lens.toml already exists. Use --force to overwrite.");
        }
        return Ok(1);
    }

    let preset = match preset {
        Some(p) => p,
        None if yes || !std::io::stdin().is_terminal() => Preset::Indie,
        None => match prompt_preset()? {
            Some(p) => p,
            None => return Ok(0), // user declined at the confirmation prompt
        },
    };

    std::fs::write(&config_path, preset.template())
        .with_context(|| format!("failed to write {}", config_path.display()))?;

    if json {
        print_json(&serde_json::json!({
            "written": true,
            "path": ".uasset-lens.toml",
            "preset": preset.name(),
        }))?;
    } else {
        println!("Wrote .uasset-lens.toml (preset: {})", preset.name());
        println!();
        println!("Tip: add .uasset-lens/ to your .gitignore to exclude the local database.");
    }
    Ok(0)
}

fn print_json(value: &serde_json::Value) -> anyhow::Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).context("Failed to serialize init output to JSON")?
    );
    Ok(())
}

/// Interactive scale + confirmation prompts (stderr). Returns `None` when the user declines.
fn prompt_preset() -> anyhow::Result<Option<Preset>> {
    use std::io::Write;

    eprint!("Project scale? [indie/mid/aaa] (default: indie): ");
    std::io::stderr().flush().ok();
    let mut scale = String::new();
    std::io::stdin().read_line(&mut scale)?;
    // Empty or unrecognized input falls back to the documented default.
    let preset = parse_preset(&scale).unwrap_or(Preset::Indie);

    eprint!("Write .uasset-lens.toml? [Y/n]: ");
    std::io::stderr().flush().ok();
    let mut confirm = String::new();
    std::io::stdin().read_line(&mut confirm)?;
    if confirm.trim().eq_ignore_ascii_case("n") {
        return Ok(None);
    }
    Ok(Some(preset))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CheckSeverity, ConfigFile};

    fn parse(preset: Preset) -> ConfigFile {
        toml::from_str(preset.template()).expect("preset template must be valid config TOML")
    }

    #[test]
    fn indie_template_should_parse_into_expected_config() {
        let cfg = parse(Preset::Indie);
        assert_eq!(cfg.rules.dead_assets, Some(CheckSeverity::Off));
        assert!(cfg.lint.naming.enabled);
        assert!(!cfg.lint.blueprint.enabled);
        assert_eq!(
            cfg.budget.limits.get("Texture2D").map(|b| b.max_size),
            Some(8 * 1024 * 1024)
        );
        assert_eq!(
            cfg.budget.limits.get("Blueprint").map(|b| b.max_size),
            Some(2 * 1024 * 1024)
        );
        assert!(cfg.check.baseline_path.is_none());
    }

    #[test]
    fn mid_template_should_parse_into_expected_config() {
        let cfg = parse(Preset::Mid);
        assert_eq!(cfg.rules.dead_assets, Some(CheckSeverity::Warn));
        assert_eq!(cfg.rules.circular_deps, Some(CheckSeverity::Error));
        assert!(cfg.lint.blueprint.enabled);
        assert_eq!(
            cfg.budget.limits.get("Texture2D").map(|b| b.max_size),
            Some(6 * 1024 * 1024)
        );
    }

    #[test]
    fn aaa_template_should_parse_into_expected_config() {
        let cfg = parse(Preset::Aaa);
        assert_eq!(cfg.rules.dead_assets, Some(CheckSeverity::Error));
        assert!(cfg.lint.blueprint.enabled);
        assert_eq!(
            cfg.budget.limits.get("Texture2D").map(|b| b.max_size),
            Some(4 * 1024 * 1024)
        );
        assert_eq!(
            cfg.check.baseline_path.as_deref(),
            Some(std::path::Path::new(".uasset-lens/baseline.json"))
        );
    }

    #[test]
    fn parse_preset_should_map_names_case_insensitively_and_reject_unknown() {
        assert_eq!(parse_preset("indie"), Some(Preset::Indie));
        assert_eq!(parse_preset(" MID "), Some(Preset::Mid));
        assert_eq!(parse_preset("AAA"), Some(Preset::Aaa));
        assert_eq!(parse_preset("huge"), None);
        assert_eq!(parse_preset(""), None);
    }

    fn init_dir(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_init_{}_{}", tag, std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn handle_init_should_write_preset_file_when_preset_given() {
        let dir = init_dir("preset");
        let result = handle_init(&dir, Some(Preset::Mid), false, false, &FormatKind::Text).unwrap();
        assert_eq!(result, 0);
        let written = std::fs::read_to_string(dir.join(".uasset-lens.toml")).unwrap();
        assert!(written.contains("--preset mid"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_init_should_exit_1_when_config_exists_without_force() {
        let dir = init_dir("exists");
        std::fs::write(dir.join(".uasset-lens.toml"), "# pre-existing\n").unwrap();
        let result =
            handle_init(&dir, Some(Preset::Indie), false, false, &FormatKind::Text).unwrap();
        assert_eq!(result, 1);
        // Existing file is left untouched.
        assert_eq!(
            std::fs::read_to_string(dir.join(".uasset-lens.toml")).unwrap(),
            "# pre-existing\n"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_init_should_overwrite_when_force() {
        let dir = init_dir("force");
        std::fs::write(dir.join(".uasset-lens.toml"), "# pre-existing\n").unwrap();
        let result = handle_init(&dir, Some(Preset::Aaa), true, false, &FormatKind::Text).unwrap();
        assert_eq!(result, 0);
        let written = std::fs::read_to_string(dir.join(".uasset-lens.toml")).unwrap();
        assert!(written.contains("--preset aaa") && written.contains("baseline_path"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_init_should_default_to_indie_when_yes_and_no_preset() {
        let dir = init_dir("yes");
        let result = handle_init(&dir, None, false, true, &FormatKind::Text).unwrap();
        assert_eq!(result, 0);
        let written = std::fs::read_to_string(dir.join(".uasset-lens.toml")).unwrap();
        assert!(written.contains("--preset indie"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
