//! `doctor` — installation health check. Runs five sequential checks (DB, schema, config,
//! scan freshness, scanner compat) and reports each as pass / fail / skipped. Checks that
//! depend on the DB being openable are skipped (`—`) when it is missing. The exit code is
//! `1` when any check fails, `2` only on a genuine I/O error opening the DB.

use std::path::{Path, PathBuf};

use uasset_lens_asset_db::{AssetDb, CURRENT_SCHEMA_VERSION, DbError};

use crate::FormatKind;
use crate::time::{format_rfc3339, format_utc};

#[derive(PartialEq)]
enum Status {
    Pass,
    Fail,
    Skip,
}

impl Status {
    fn symbol(&self) -> &'static str {
        match self {
            Status::Pass => "✓",
            Status::Fail => "✗",
            Status::Skip => "—",
        }
    }
}

struct Line {
    label: &'static str,
    status: Status,
    detail: String,
    hint: Option<String>,
}

fn days_since(ts: u64) -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(ts);
    now.saturating_sub(ts) / 86400
}

pub fn handle_doctor(
    project_dir: Option<&Path>,
    db_override: Option<&Path>,
    config_override: Option<&Path>,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    if matches!(format, FormatKind::Sarif) {
        return Err(crate::sarif_not_supported());
    }
    let json = matches!(format, FormatKind::Json);
    let tool_version = env!("CARGO_PKG_VERSION");

    let dir = project_dir.unwrap_or(Path::new("."));
    let db_path = crate::resolve_db_path(dir, db_override);
    let config_path: PathBuf = match config_override {
        Some(p) => p.to_path_buf(),
        None => dir.join(".uasset-lens.toml"),
    };

    // [DB] — a missing file is a failed check (exit 1); any other open error is an execution
    // error (exit 2), reported on stderr with no stdout output (cli-output.md).
    let db = match AssetDb::open_existing(&db_path) {
        Ok(d) => Some(d),
        Err(DbError::NotFound(_)) => None,
        Err(e) => {
            eprintln!("error: failed to open database {}: {e}", db_path.display());
            return Ok(2);
        }
    };

    let asset_count = db.as_ref().and_then(|d| d.asset_count().ok());
    let schema_version = db.as_ref().and_then(|d| d.schema_version().ok());
    let last_scan = db
        .as_ref()
        .and_then(|d| d.recent_snapshots(1).ok())
        .and_then(|v| v.into_iter().next());
    let db_scanner = db.as_ref().and_then(|d| d.scanner_version().ok()).flatten();

    // [DB]
    let db_line = match &db {
        Some(_) => Line {
            label: "[DB]",
            status: Status::Pass,
            detail: format!(
                "{} ({} assets)",
                db_path.display(),
                asset_count.unwrap_or(0)
            ),
            hint: None,
        },
        None => Line {
            label: "[DB]",
            status: Status::Fail,
            detail: format!("{} not found", db_path.display()),
            hint: Some("Run 'uasset-lens scan <project_dir>' to create it.".to_string()),
        },
    };

    // [Schema]
    let schema_line = match &db {
        None => Line {
            label: "[Schema]",
            status: Status::Skip,
            detail: "Cannot check (DB missing)".to_string(),
            hint: None,
        },
        Some(_) if schema_version == Some(CURRENT_SCHEMA_VERSION) => Line {
            label: "[Schema]",
            status: Status::Pass,
            detail: format!("v{CURRENT_SCHEMA_VERSION} (current)"),
            hint: None,
        },
        Some(_) => Line {
            label: "[Schema]",
            status: Status::Fail,
            detail: format!(
                "v{} is outdated (expected v{CURRENT_SCHEMA_VERSION})",
                schema_version.unwrap_or(0)
            ),
            hint: Some("Run 'uasset-lens scan --full-scan <project_dir>' to migrate.".to_string()),
        },
    };

    // [Config] — absent config is not an error; the tool falls back to defaults.
    let config_present = config_path.exists();
    let config_parses = config_present
        && std::fs::read_to_string(&config_path)
            .ok()
            .is_some_and(|s| toml::from_str::<toml::Value>(&s).is_ok());
    let config_line = if !config_present {
        Line {
            label: "[Config]",
            status: Status::Skip,
            detail: format!(
                "{} not found (defaults will be used)",
                config_path.display()
            ),
            hint: None,
        }
    } else if config_parses {
        Line {
            label: "[Config]",
            status: Status::Pass,
            detail: format!("{} (valid)", config_path.display()),
            hint: None,
        }
    } else {
        Line {
            label: "[Config]",
            status: Status::Fail,
            detail: format!("{} (parse error)", config_path.display()),
            hint: Some("Run 'uasset-lens config validate' for details.".to_string()),
        }
    };

    // [Scan]
    let scan_line = match (&db, &last_scan) {
        (None, _) => Line {
            label: "[Scan]",
            status: Status::Skip,
            detail: "Cannot check (DB missing)".to_string(),
            hint: None,
        },
        (Some(_), Some(s)) => Line {
            label: "[Scan]",
            status: Status::Pass,
            detail: format!(
                "Last scan: {} ({} days ago)",
                format_utc(s.scanned_at),
                days_since(s.scanned_at)
            ),
            hint: None,
        },
        (Some(_), None) => Line {
            label: "[Scan]",
            status: Status::Fail,
            detail: "No scan data available.".to_string(),
            hint: Some("Run 'uasset-lens scan <project_dir>'.".to_string()),
        },
    };

    // [Compat]
    let compat_line = match &db {
        None => Line {
            label: "[Compat]",
            status: Status::Skip,
            detail: "Cannot check (DB missing)".to_string(),
            hint: None,
        },
        Some(_) if db_scanner.as_deref() == Some(tool_version) => Line {
            label: "[Compat]",
            status: Status::Pass,
            detail: format!("Scanner version v{tool_version} matches binary"),
            hint: None,
        },
        Some(_) => Line {
            label: "[Compat]",
            status: Status::Fail,
            detail: match &db_scanner {
                Some(v) => format!(
                    "Scanner version mismatch: DB built with v{v}, current binary is v{tool_version}"
                ),
                None => "Scanner version not recorded (re-scan to record).".to_string(),
            },
            hint: None,
        },
    };

    let lines = [db_line, schema_line, config_line, scan_line, compat_line];
    let issues = lines.iter().filter(|l| l.status == Status::Fail).count();

    if json {
        let value = serde_json::json!({
            "tool_version": tool_version,
            "checks": {
                "db": {
                    "passed": db.is_some(),
                    "schema_version": schema_version,
                    "asset_count": asset_count,
                },
                "schema": {
                    "passed": lines[1].status == Status::Pass,
                    "skipped": lines[1].status == Status::Skip,
                    "version": schema_version,
                    "expected": CURRENT_SCHEMA_VERSION,
                },
                "config": {
                    "passed": config_line_passed(&lines[2]),
                    "present": config_present,
                },
                "scan": {
                    "passed": lines[3].status == Status::Pass,
                    "skipped": lines[3].status == Status::Skip,
                    "last_scan_utc": last_scan.as_ref().map(|s| format_rfc3339(s.scanned_at)),
                    "days_since_scan": last_scan.as_ref().map(|s| days_since(s.scanned_at)),
                },
                "compat": {
                    "passed": lines[4].status == Status::Pass,
                    "skipped": lines[4].status == Status::Skip,
                    "db_scanner_version": db_scanner,
                    "binary_version": tool_version,
                },
            },
            "issues_found": issues,
        });
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        print_text(tool_version, &lines, issues);
    }

    Ok(if issues == 0 { 0 } else { 1 })
}

// An absent config (Skip) is healthy — defaults apply — so it reports `passed: true` in JSON.
fn config_line_passed(line: &Line) -> bool {
    line.status != Status::Fail
}

fn print_text(tool_version: &str, lines: &[Line], issues: usize) {
    println!("uasset-lens v{tool_version}");
    println!();
    for line in lines {
        println!("{:<9}{}  {}", line.label, line.status.symbol(), line.detail);
        if let Some(hint) = &line.hint {
            println!("             {hint}");
        }
    }
    println!();
    if issues == 0 {
        println!("All checks passed.");
    } else if issues == 1 {
        println!("1 issue found.");
    } else {
        println!("{issues} issues found.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uasset_lens_shared::AssetType;

    // Creates an empty project dir with the `.uasset-lens` marker; returns (dir, db_path).
    fn empty_project_dir(tag: &str) -> (PathBuf, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_doctor_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".uasset-lens")).unwrap();
        let db_path = crate::resolve_db_path(&dir, None);
        (dir, db_path)
    }

    fn sample_meta() -> uasset_lens_scanner::AssetMetadata {
        crate::commands::make_meta(
            "/Game/A",
            PathBuf::from("a.uasset"),
            AssetType::Blueprint,
            100,
            vec![],
        )
    }

    // Creates a project dir with a populated DB (one asset, one snapshot, scanner version stamped).
    fn project_with_scanned_db(tag: &str, scanner_version: &str) -> PathBuf {
        let (dir, db_path) = empty_project_dir(tag);
        let db = AssetDb::open(&db_path).unwrap();
        db.upsert_asset(&sample_meta()).unwrap();
        db.record_scan_snapshot().unwrap();
        db.set_scanner_version(scanner_version).unwrap();
        drop(db);
        dir
    }

    #[test]
    fn handle_doctor_should_exit_0_when_all_checks_pass() {
        let dir = project_with_scanned_db("allpass", env!("CARGO_PKG_VERSION"));
        let code = handle_doctor(Some(&dir), None, None, &FormatKind::Text).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(code, 0);
    }

    #[test]
    fn handle_doctor_should_exit_1_when_db_missing() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_doctor_nodb_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let code = handle_doctor(Some(&dir), None, None, &FormatKind::Text).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(code, 1);
    }

    #[test]
    fn handle_doctor_should_exit_1_when_scanner_version_mismatches() {
        let dir = project_with_scanned_db("mismatch", "0.0.1-old");
        let code = handle_doctor(Some(&dir), None, None, &FormatKind::Text).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(code, 1);
    }

    #[test]
    fn handle_doctor_should_exit_1_when_schema_outdated() {
        let dir = project_with_scanned_db("schemaold", env!("CARGO_PKG_VERSION"));
        // Force a schema version different from CURRENT to simulate an outdated DB. A non-zero
        // value is required: init_schema would re-stamp user_version 0 back to CURRENT on reopen.
        let db = AssetDb::open(&crate::resolve_db_path(&dir, None)).unwrap();
        uasset_lens_asset_db::set_schema_version(
            &db,
            uasset_lens_asset_db::CURRENT_SCHEMA_VERSION + 1,
        );
        drop(db);
        let code = handle_doctor(Some(&dir), None, None, &FormatKind::Text).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(code, 1);
    }

    #[test]
    fn handle_doctor_should_exit_1_when_scanner_version_not_recorded() {
        // A DB scanned before version tracking has no scanner_version → [Compat] fails.
        let (dir, db_path) = empty_project_dir("nocompat");
        let db = AssetDb::open(&db_path).unwrap();
        db.upsert_asset(&sample_meta()).unwrap();
        db.record_scan_snapshot().unwrap();
        drop(db);
        let code = handle_doctor(Some(&dir), None, None, &FormatKind::Text).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(code, 1);
    }

    #[test]
    fn handle_doctor_should_exit_1_when_no_scan_snapshot() {
        // A DB with assets but no scan_history row → [Scan] fails.
        let (dir, db_path) = empty_project_dir("nosnap");
        let db = AssetDb::open(&db_path).unwrap();
        db.upsert_asset(&sample_meta()).unwrap();
        db.set_scanner_version(env!("CARGO_PKG_VERSION")).unwrap();
        drop(db);
        let code = handle_doctor(Some(&dir), None, None, &FormatKind::Text).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(code, 1);
    }

    #[test]
    fn handle_doctor_should_exit_1_when_config_is_malformed() {
        let dir = project_with_scanned_db("badconfig", env!("CARGO_PKG_VERSION"));
        std::fs::write(dir.join(".uasset-lens.toml"), "this is = = not toml").unwrap();
        let code = handle_doctor(Some(&dir), None, None, &FormatKind::Text).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(code, 1);
    }

    #[test]
    fn handle_doctor_json_should_pass_with_valid_config_present() {
        let dir = project_with_scanned_db("jsoncfg", env!("CARGO_PKG_VERSION"));
        std::fs::write(dir.join(".uasset-lens.toml"), "[scan]\n").unwrap();
        let code = handle_doctor(Some(&dir), None, None, &FormatKind::Json).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(code, 0);
    }

    #[test]
    fn days_since_should_return_zero_for_now() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(days_since(now), 0);
    }
}
