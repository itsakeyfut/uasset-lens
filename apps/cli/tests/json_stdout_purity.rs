//! Regression tests for the "JSON stdout leak" footgun (RK-017): a command supporting
//! `--format json` must write a single JSON value to stdout, with no human-readable lines
//! leaking ahead of it.
//!
//! - `scan` was the original offender (#365): per-category count lines were printed to stdout
//!   regardless of format.
//! - The second test guards the remaining `--format json` commands against the same pattern.

use std::path::Path;
use std::process::Command;

/// Creates a temp project with one fixture asset and scans it so DB-reading commands have data.
/// Returns the project directory.
fn scanned_project(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "uasset_lens_jsonpurity_{}_{}",
        tag,
        std::process::id()
    ));
    let content = dir.join("Content");
    std::fs::create_dir_all(&content).unwrap();
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/valid/BP_Simple.uasset"
    );
    std::fs::copy(fixture, content.join("BP_Hero.uasset")).unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_uasset-lens"))
        .args(["scan", dir.to_str().unwrap()])
        .status()
        .expect("failed to run scan");
    assert!(status.success(), "scan setup must succeed");
    dir
}

/// Asserts the command's stdout is a single JSON value (starts with `{` or `[` after trimming).
fn assert_json_stdout(dir: &Path, args: &[&str]) {
    let output = Command::new(env!("CARGO_BIN_EXE_uasset-lens"))
        .args(args)
        .arg(dir)
        .args(["--format", "json"])
        .output()
        .unwrap_or_else(|e| panic!("{args:?} failed to run: {e}"));
    let stdout = String::from_utf8(output.stdout).expect("stdout is not UTF-8");
    let trimmed = stdout.trim();
    let first = trimmed
        .chars()
        .next()
        .unwrap_or_else(|| panic!("{args:?} --format json produced empty stdout"));
    assert!(
        first == '{' || first == '[',
        "{args:?} --format json stdout must be a single JSON value (start '{{' or '['), got:\n{stdout}"
    );
}

#[test]
fn scan_format_json_should_emit_only_json_to_stdout() {
    let dir = scanned_project("scan");
    let output = Command::new(env!("CARGO_BIN_EXE_uasset-lens"))
        .args(["scan", dir.to_str().unwrap(), "--format", "json"])
        .output()
        .expect("failed to run the uasset-lens binary");
    let _ = std::fs::remove_dir_all(&dir);

    let stdout = String::from_utf8(output.stdout).expect("stdout is not UTF-8");
    // The per-category count lines (printed to stdout in text mode) must not leak in JSON mode.
    assert!(
        !stdout.contains("new asset(s) indexed"),
        "per-category count lines leaked into JSON stdout:\n{stdout}"
    );
    let trimmed = stdout.trim();
    assert!(
        trimmed.starts_with('{') && trimmed.ends_with('}'),
        "scan --format json stdout must be a single JSON object:\n{stdout}"
    );
}

#[test]
fn query_commands_format_json_should_emit_only_json_to_stdout() {
    let dir = scanned_project("query");
    // Every `--format json` command that takes just <project_dir> must emit a single JSON value.
    for cmd in [
        "stats",
        "graph",
        "dead-assets",
        "redirectors",
        "duplicates",
        "budget",
        "lint",
        "blueprint",
        "find",
        "check",
        "doctor",
    ] {
        assert_json_stdout(&dir, &[cmd]);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
