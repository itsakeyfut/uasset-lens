use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::FormatKind;

#[derive(serde::Serialize)]
struct PathConvOutput {
    input: String,
    output: String,
}

pub fn handle_path_conv(
    input: &str,
    to_file: bool,
    content_root_override: Option<&Path>,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    let output = if to_file || input.starts_with("/Game/") {
        let cr = resolve_content_root_for_game_path(content_root_override)?;
        let rel = input.strip_prefix("/Game/").unwrap_or(input);
        cr.join(rel).with_extension("uasset").display().to_string()
    } else {
        let cr = resolve_content_root_for_fs_path(input, content_root_override)?;
        let asset_path = shared::AssetPath::from_fs_path(&cr, Path::new(input))
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        asset_path.as_str().to_owned()
    };

    match format {
        FormatKind::Json => {
            let out = PathConvOutput {
                input: input.to_owned(),
                output,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&out).context("Failed to serialize path output")?
            );
        }
        FormatKind::GithubActions | FormatKind::Text => println!("{output}"),
    }

    Ok(0)
}

fn resolve_content_root_for_fs_path(
    input: &str,
    override_path: Option<&Path>,
) -> anyhow::Result<PathBuf> {
    if let Some(cr) = override_path {
        return Ok(cr.to_path_buf());
    }
    let mut accumulated = PathBuf::new();
    for component in Path::new(input).components() {
        accumulated.push(component);
        if component.as_os_str() == "Content" {
            return Ok(accumulated);
        }
    }
    anyhow::bail!(
        "cannot determine content root from path; use --content-root to specify it explicitly"
    )
}

fn resolve_content_root_for_game_path(override_path: Option<&Path>) -> anyhow::Result<PathBuf> {
    if let Some(cr) = override_path {
        return Ok(cr.to_path_buf());
    }
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let project_dir = find_project_dir(&cwd)?;
    Ok(crate::resolve_content_root(&project_dir))
}

fn find_project_dir(start: &Path) -> anyhow::Result<PathBuf> {
    let mut dir = start;
    loop {
        if dir.join(".uasset-lens").is_dir() {
            return Ok(dir.to_path_buf());
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => {
                anyhow::bail!("no scan data found.\nRun 'uasset-lens scan <project_dir>' first.")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_content_dir(base: &Path) -> PathBuf {
        let content = base.join("Content");
        std::fs::create_dir_all(&content).unwrap();
        content
    }

    #[test]
    fn handle_path_conv_should_convert_fs_path_to_game_path() {
        let dir =
            std::env::temp_dir().join(format!("uasset_path_conv_fs2game_{}", std::process::id()));
        let content = make_content_dir(&dir);
        let chars = content.join("Characters");
        std::fs::create_dir_all(&chars).unwrap();
        let file = chars.join("BP_Hero.uasset");
        std::fs::write(&file, b"").unwrap();

        let result = handle_path_conv(
            file.to_str().unwrap(),
            false,
            Some(&content),
            &FormatKind::Text,
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn handle_path_conv_should_convert_game_path_to_fs_path() {
        let dir =
            std::env::temp_dir().join(format!("uasset_path_conv_game2fs_{}", std::process::id()));
        let content = make_content_dir(&dir);

        let result = handle_path_conv(
            "/Game/Characters/BP_Hero",
            false,
            Some(&content),
            &FormatKind::Text,
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn handle_path_conv_to_file_flag_should_force_game_to_fs_conversion() {
        let dir =
            std::env::temp_dir().join(format!("uasset_path_conv_to_file_{}", std::process::id()));
        let content = make_content_dir(&dir);

        let result = handle_path_conv(
            "/Game/Chars/T_Rock",
            true,
            Some(&content),
            &FormatKind::Text,
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn handle_path_conv_should_return_err_when_fs_path_outside_content_root() {
        let dir =
            std::env::temp_dir().join(format!("uasset_path_conv_outside_{}", std::process::id()));
        let content = make_content_dir(&dir);
        let other = dir.join("Other");
        std::fs::create_dir_all(&other).unwrap();
        let file = other.join("BP_Hero.uasset");
        std::fs::write(&file, b"").unwrap();

        let result = handle_path_conv(
            file.to_str().unwrap(),
            false,
            Some(&content),
            &FormatKind::Text,
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_err());
    }

    #[test]
    fn handle_path_conv_should_auto_detect_content_root_from_fs_path() {
        let dir =
            std::env::temp_dir().join(format!("uasset_path_conv_auto_{}", std::process::id()));
        let content = make_content_dir(&dir);
        let chars = content.join("Characters");
        std::fs::create_dir_all(&chars).unwrap();
        let file = chars.join("BP_Hero.uasset");
        std::fs::write(&file, b"").unwrap();

        let result = handle_path_conv(file.to_str().unwrap(), false, None, &FormatKind::Text);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn handle_path_conv_should_return_err_when_no_content_segment_and_no_override() {
        let result = handle_path_conv(
            "/tmp/some/path/BP_Hero.uasset",
            false,
            None,
            &FormatKind::Text,
        );
        assert!(result.is_err());
    }

    #[test]
    fn handle_path_conv_json_should_succeed() {
        let dir =
            std::env::temp_dir().join(format!("uasset_path_conv_json_{}", std::process::id()));
        let content = make_content_dir(&dir);
        let chars = content.join("Characters");
        std::fs::create_dir_all(&chars).unwrap();
        let file = chars.join("BP_Hero.uasset");
        std::fs::write(&file, b"").unwrap();

        let result = handle_path_conv(
            file.to_str().unwrap(),
            false,
            Some(&content),
            &FormatKind::Json,
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(result.unwrap(), 0);
    }
}
