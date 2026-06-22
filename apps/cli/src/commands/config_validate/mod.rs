//! `config validate` — checks `.uasset-lens.toml` for TOML syntax, field types, value
//! constraints, and unknown fields. Validation walks the raw `toml::Value` tree (rather than
//! deserializing into `ConfigFile`) so that ALL issues are collected in one pass; serde stops
//! at the first error. Line numbers are best-effort (`Option`): a raw-source scan for the key.

mod validator;

use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::FormatKind;

use validator::validate;

#[derive(Debug)]
struct ConfigError {
    line: Option<u32>,
    section: String,
    message: String,
}

#[derive(Debug)]
struct ConfigWarning {
    line: Option<u32>,
    section: String,
    message: String,
    suggestion: Option<String>,
}

impl ConfigWarning {
    /// Folds the typo suggestion into the message (matching the spec's text/JSON output).
    fn rendered(&self) -> String {
        match &self.suggestion {
            Some(s) => format!("{} (did you mean '{}'?)", self.message, s),
            None => self.message.clone(),
        }
    }
}

/// Levenshtein edit distance (iterative two-row DP).
fn levenshtein(a: &str, b: &str) -> usize {
    let b_chars: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b_chars.len()).collect();
    let mut curr = vec![0usize; b_chars.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, &cb) in b_chars.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b_chars.len()]
}

/// The closest known key within Levenshtein distance 2, if any.
fn closest<'a>(key: &str, known: &[&'a str]) -> Option<&'a str> {
    known
        .iter()
        .map(|k| (*k, levenshtein(key, k)))
        .filter(|(_, d)| *d <= 2)
        .min_by_key(|(_, d)| *d)
        .map(|(k, _)| k)
}

/// Best-effort 1-based line number of `key` in the raw source: the first line whose first token
/// (ignoring a leading `[`) is `key`. Returns `None` if not found.
fn find_line(raw: &str, key: &str) -> Option<u32> {
    for (i, line) in raw.lines().enumerate() {
        let t = line.trim_start().trim_start_matches('[');
        if let Some(rest) = t.strip_prefix(key) {
            let next = rest.chars().next();
            if matches!(next, None | Some('=' | '.' | ']' | ' ' | '\t')) {
                return Some(i as u32 + 1);
            }
        }
    }
    None
}

/// 1-based line number for a byte offset into `raw`.
fn line_of_offset(raw: &str, offset: usize) -> u32 {
    raw.get(..offset)
        .map(|s| s.matches('\n').count() as u32 + 1)
        .unwrap_or(1)
}

pub fn handle_config_validate(
    project_dir: Option<&Path>,
    config_override: Option<&Path>,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    if matches!(format, FormatKind::Sarif) {
        return Err(crate::sarif_not_supported());
    }
    let json = matches!(format, FormatKind::Json);

    let path: PathBuf = match config_override {
        Some(p) => p.to_path_buf(),
        None => project_dir
            .unwrap_or(Path::new("."))
            .join(".uasset-lens.toml"),
    };
    let path_str = path.display().to_string();

    if !path.exists() {
        if json {
            print_json(&serde_json::json!({
                "valid": true,
                "path": path_str,
                "present": false,
                "errors": [],
                "warnings": [],
            }))?;
        } else {
            println!("No config file found at {path_str} (using defaults).");
        }
        return Ok(0);
    }

    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to read {path_str}: {e}");
            return Ok(2);
        }
    };

    let value: toml::Value = match toml::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            let line = e.span().map(|sp| line_of_offset(&raw, sp.start));
            // exit 2 is an execution error: report on stderr, no JSON object (cli-output.md).
            eprintln!("error: {path_str} failed to parse (exit 2)");
            eprintln!();
            match line {
                Some(n) => eprintln!("  line {n}: {}", e.message()),
                None => eprintln!("  {}", e.message()),
            }
            return Ok(2);
        }
    };

    let (errors, warnings) = validate(&value, &raw);

    if json {
        let errs: Vec<_> = errors
            .iter()
            .map(|e| serde_json::json!({ "line": e.line, "section": e.section, "message": e.message }))
            .collect();
        let warns: Vec<_> = warnings
            .iter()
            .map(|w| serde_json::json!({ "line": w.line, "section": w.section, "message": w.rendered() }))
            .collect();
        print_json(&serde_json::json!({
            "valid": errors.is_empty(),
            "path": path_str,
            "errors": errs,
            "warnings": warns,
        }))?;
    } else {
        print_text(&path_str, &errors, &warnings);
    }

    Ok(if errors.is_empty() { 0 } else { 1 })
}

fn print_text(path_str: &str, errors: &[ConfigError], warnings: &[ConfigWarning]) {
    if errors.is_empty() {
        println!("{path_str} is valid.");
        if !warnings.is_empty() {
            println!();
            println!("  {} warning(s):", warnings.len());
            for w in warnings {
                println!("    {}", format_entry(w.line, &w.section, &w.rendered()));
            }
        }
    } else {
        println!("{path_str}: {} error(s) found", errors.len());
        println!();
        // Errors and warnings interleaved in ascending line order (file order).
        let mut entries: Vec<(Option<u32>, String)> = errors
            .iter()
            .map(|e| (e.line, format_entry(e.line, &e.section, &e.message)))
            .chain(
                warnings
                    .iter()
                    .map(|w| (w.line, format_entry(w.line, &w.section, &w.rendered()))),
            )
            .collect();
        entries.sort_by_key(|(line, _)| line.unwrap_or(u32::MAX));
        for (_, text) in &entries {
            println!("  {text}");
        }
    }
}

fn format_entry(line: Option<u32>, section: &str, message: &str) -> String {
    match line {
        Some(n) => format!("line {n}: [{section}] — {message}"),
        None => format!("[{section}] — {message}"),
    }
}

fn print_json(value: &serde_json::Value) -> anyhow::Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(value)
            .context("Failed to serialize config validate output")?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levenshtein_should_compute_edit_distance() {
        assert_eq!(levenshtein("scan", "scn"), 1);
        assert_eq!(levenshtein("exclude_path", "exclude_paths"), 1);
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", "abc"), 0);
    }

    #[test]
    fn find_line_should_locate_keys_and_sections() {
        let raw = "[scan]\nexclude_paths = []\ncontent_root = \"Content\"\n";
        assert_eq!(find_line(raw, "scan"), Some(1));
        assert_eq!(find_line(raw, "exclude_paths"), Some(2));
        assert_eq!(find_line(raw, "content_root"), Some(3));
        assert_eq!(find_line(raw, "nonexistent"), None);
    }

    /// Creates a temp project dir, optionally writing `.uasset-lens.toml` with `content`.
    fn project_dir(tag: &str, content: Option<&str>) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_cfgval_{}_{}", tag, std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        match content {
            Some(c) => std::fs::write(dir.join(".uasset-lens.toml"), c).unwrap(),
            None => {
                let _ = std::fs::remove_file(dir.join(".uasset-lens.toml"));
            }
        }
        dir
    }

    fn exit_code(dir: &Path) -> i32 {
        let code = handle_config_validate(Some(dir), None, &FormatKind::Text).unwrap();
        let _ = std::fs::remove_dir_all(dir);
        code
    }

    #[test]
    fn handle_config_validate_should_exit_0_for_valid_config() {
        let dir = project_dir("valid", Some("[budget]\nTexture2D.max_size = 4194304\n"));
        assert_eq!(exit_code(&dir), 0);
    }

    #[test]
    fn handle_config_validate_should_exit_1_for_validation_errors() {
        let dir = project_dir("invalid", Some("[budget]\nTexture2D.max_size = 0\n"));
        assert_eq!(exit_code(&dir), 1);
    }

    #[test]
    fn handle_config_validate_should_exit_0_when_config_absent() {
        let dir = project_dir("absent", None);
        assert_eq!(exit_code(&dir), 0);
    }

    #[test]
    fn handle_config_validate_should_exit_2_on_parse_error() {
        let dir = project_dir("parse", Some("[scan\nexclude_paths = ]\n"));
        assert_eq!(exit_code(&dir), 2);
    }

    #[test]
    fn handle_config_validate_should_use_config_override() {
        let dir = project_dir("override", None);
        let custom = dir.join("custom.toml");
        std::fs::write(&custom, "[rules]\ndead-assets = \"warn\"\n").unwrap();
        let code = handle_config_validate(None, Some(&custom), &FormatKind::Text).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(code, 0);
    }
}
