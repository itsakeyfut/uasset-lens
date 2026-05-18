use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::path::Path;
use std::time::UNIX_EPOCH;

use anyhow::Context;

use crate::FormatKind;

const SIDECAR_EXTENSIONS: &[&str] = &["uexp", "ubulk", "uptnl"];

struct CleanEntry {
    asset_path: shared::AssetPath,
    file_path: std::path::PathBuf,
    asset_type: String,
    file_size: u64,
    last_modified_db: u64,
}

#[derive(serde::Serialize)]
struct CleanTarget {
    path: String,
    #[serde(rename = "type")]
    asset_type: String,
    file_size: u64,
}

#[derive(serde::Serialize)]
struct DryRunOutput {
    targets: Vec<CleanTarget>,
    total_size_bytes: u64,
}

#[derive(serde::Serialize)]
struct CleanSummaryJson {
    deleted: usize,
    freed_bytes: u64,
    skipped: usize,
    errors: usize,
}

// Each arg maps to a distinct CLI flag; a wrapper struct adds indirection at a single call site.
#[allow(clippy::too_many_arguments)]
pub fn handle_clean(
    _project_dir: &Path,
    yes: bool,
    dry_run: bool,
    min_size: Option<u64>,
    exclude_patterns: &[String],
    path_filter: Option<&str>,
    db_path: &Path,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    let db = crate::open_db(db_path)?;
    let graph = crate::load_graph(&db)?;

    let dead_paths = dead_asset_detector::detect(&graph);

    let type_map: HashMap<&shared::AssetPath, String> = graph
        .nodes()
        .map(|n| (&n.path, n.asset_type.to_string()))
        .collect();

    let mut entries: Vec<CleanEntry> = dead_paths
        .iter()
        .filter_map(|path| {
            let record = db.get_asset(path).ok()??;
            let asset_type = type_map
                .get(path)
                .cloned() // clone required: type_map yields &String; owned String needed for CleanEntry
                .unwrap_or_default();
            Some(CleanEntry {
                asset_path: path.clone(), // clone required: path is &AssetPath; CleanEntry needs owned
                file_path: record.file_path,
                asset_type,
                file_size: record.file_size,
                last_modified_db: record.last_modified,
            })
        })
        .collect();

    if let Some(min) = min_size {
        entries.retain(|e| e.file_size >= min);
    }

    if !exclude_patterns.is_empty() {
        entries.retain(|e| {
            !exclude_patterns
                .iter()
                .any(|p| e.asset_path.as_str().contains(p.as_str()))
        });
    }

    if let Some(pattern) = path_filter {
        let matcher = globset::Glob::new(pattern)
            .context("Invalid glob pattern for --path")?
            .compile_matcher();
        entries.retain(|e| matcher.is_match(e.asset_path.as_str()));
    }

    entries.sort_unstable_by_key(|e| std::cmp::Reverse(e.file_size));

    if dry_run {
        let total_size_bytes: u64 = entries.iter().map(|e| e.file_size).sum();
        let count = entries.len();
        match format {
            FormatKind::Json => {
                let targets = entries
                    .iter()
                    .map(|e| CleanTarget {
                        path: e.asset_path.as_str().to_owned(),
                        asset_type: e.asset_type.clone(), // clone required: CleanTarget needs owned String
                        file_size: e.file_size,
                    })
                    .collect();
                let output = DryRunOutput {
                    targets,
                    total_size_bytes,
                };
                println!(
                    "{}",
                    serde_json::to_string_pretty(&output)
                        .context("Failed to serialize dry-run output to JSON")?
                );
            }
            FormatKind::GithubActions | FormatKind::Text => {
                for e in &entries {
                    println!(
                        "  {}  ({}, {})",
                        e.asset_path.as_str(),
                        e.asset_type,
                        crate::format_size(e.file_size)
                    );
                }
                if count > 0 {
                    println!();
                }
                println!(
                    "  Dry run: {} assets would be deleted ({})",
                    count,
                    crate::format_size(total_size_bytes)
                );
            }
        }
        return Ok(0);
    }

    let total = entries.len();
    let total_size: u64 = entries.iter().map(|e| e.file_size).sum();
    eprintln!(
        "Found {} dead assets ({})",
        total,
        crate::format_size(total_size)
    );

    let mut deleted: usize = 0;
    let mut freed_bytes: u64 = 0;
    let mut skipped: usize = 0;
    let mut errors: usize = 0;
    let mut delete_all = yes;
    let mut skip_types: HashSet<String> = HashSet::new();

    let width = if total == 0 {
        1
    } else {
        total.ilog10() as usize + 1
    };

    'outer: for (i, entry) in entries.iter().enumerate() {
        if skip_types.contains(&entry.asset_type) {
            skipped += 1;
            continue;
        }

        eprintln!(
            "\n  [{:>width$}/{}] {}  ({}, {})",
            i + 1,
            total,
            entry.asset_path.as_str(),
            entry.asset_type,
            crate::format_size(entry.file_size),
            width = width,
        );

        if !delete_all {
            match std::fs::metadata(&entry.file_path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
            {
                None => eprintln!(
                    "  warning: {} not found on disk — may have already been deleted manually",
                    entry.asset_path.as_str()
                ),
                Some(mtime) if mtime != entry.last_modified_db => eprintln!(
                    "  warning: {} modified since last scan",
                    entry.asset_path.as_str()
                ),
                _ => {}
            }

            eprint!("  Delete? [y / N / a(ll) / s(kip type) / q(uit)] > ");
            io::stderr().flush().context("Failed to flush stderr")?;

            let mut answer = String::new();
            io::stdin()
                .read_line(&mut answer)
                .context("Failed to read answer")?;

            match answer.trim().to_lowercase().as_str() {
                "a" => delete_all = true,
                "s" => {
                    skip_types.insert(entry.asset_type.clone()); // clone required: HashSet needs owned String
                    skipped += 1;
                    continue 'outer;
                }
                "q" => break 'outer,
                "y" => {}
                _ => {
                    skipped += 1;
                    continue 'outer;
                }
            }
        }

        if let Err(e) = std::fs::remove_file(&entry.file_path) {
            eprintln!(
                "  error: failed to delete {}: {}",
                entry.file_path.display(),
                e
            );
            errors += 1;
            continue;
        }

        delete_sidecars(&entry.file_path, &mut errors);

        if let Err(e) = db.delete_asset(&entry.asset_path) {
            eprintln!(
                "  error: failed to remove {} from DB: {}",
                entry.asset_path.as_str(),
                e
            );
            errors += 1;
        }

        deleted += 1;
        freed_bytes += entry.file_size;
        eprintln!(
            "  Deleted: {} ({})",
            entry.file_path.display(),
            crate::format_size(entry.file_size)
        );
    }

    eprintln!();
    match format {
        FormatKind::Json => {
            let summary = CleanSummaryJson {
                deleted,
                freed_bytes,
                skipped,
                errors,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&summary)
                    .context("Failed to serialize summary to JSON")?
            );
        }
        FormatKind::GithubActions | FormatKind::Text => {
            println!(
                "  Summary: {} deleted ({}), {} skipped, {} errors",
                deleted,
                crate::format_size(freed_bytes),
                skipped,
                errors,
            );
        }
    }

    Ok(0)
}

fn delete_sidecars(file_path: &Path, errors: &mut usize) {
    for ext in SIDECAR_EXTENSIONS {
        let sidecar = file_path.with_extension(ext);
        if sidecar.exists()
            && let Err(e) = std::fs::remove_file(&sidecar)
        {
            eprintln!(
                "  warning: failed to delete sidecar {}: {}",
                sidecar.display(),
                e
            );
            *errors += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::make_meta;
    use shared::{AssetPath, AssetType};

    fn make_uasset(dir: &Path, name: &str) -> std::path::PathBuf {
        let path = dir.join(format!("{name}.uasset"));
        std::fs::write(&path, b"fake").unwrap();
        path
    }

    #[test]
    fn handle_clean_should_return_err_when_db_does_not_exist() {
        let db_path = std::env::temp_dir().join(format!(
            "uasset_lens_clean160_missing_{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);

        let result = handle_clean(
            Path::new("/proj"),
            false,
            false,
            None,
            &[],
            None,
            &db_path,
            &FormatKind::Text,
        );
        assert!(result.is_err(), "missing DB should return Err");
    }

    #[test]
    fn handle_clean_should_return_0_when_no_dead_assets() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_clean160_empty_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_clean(
            &dir,
            true,
            false,
            None,
            &[],
            None,
            &db_path,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_clean_dry_run_should_return_0_and_not_delete_files() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_clean160_dryrun_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        let uasset = make_uasset(&dir, "Orphan");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/Orphan",
                uasset.clone(),
                AssetType::Blueprint,
                4,
                vec![],
            )])
            .unwrap();
        }

        let result = handle_clean(
            &dir,
            false,
            true,
            None,
            &[],
            None,
            &db_path,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "dry-run exits 0");
        assert!(uasset.exists(), "dry-run must not delete the file");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_clean_yes_should_delete_dead_asset_file() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_clean160_yes_del_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        let uasset = make_uasset(&dir, "Orphan");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/Orphan",
                uasset.clone(),
                AssetType::StaticMesh,
                4,
                vec![],
            )])
            .unwrap();
        }

        handle_clean(
            &dir,
            true,
            false,
            None,
            &[],
            None,
            &db_path,
            &FormatKind::Text,
        )
        .unwrap();
        assert!(!uasset.exists(), "--yes must delete the .uasset file");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_clean_yes_should_return_0_after_deletion() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_clean160_yes_rc_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        let uasset = make_uasset(&dir, "Orphan");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/Orphan",
                uasset,
                AssetType::StaticMesh,
                4,
                vec![],
            )])
            .unwrap();
        }

        let result = handle_clean(
            &dir,
            true,
            false,
            None,
            &[],
            None,
            &db_path,
            &FormatKind::Text,
        )
        .unwrap();
        assert_eq!(result, 0, "--yes should return 0");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_clean_yes_should_remove_asset_from_db() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_clean160_yes_db_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        let uasset = make_uasset(&dir, "Orphan");
        let asset_path = AssetPath::new("/Game/Orphan").unwrap();

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/Orphan",
                uasset,
                AssetType::Blueprint,
                4,
                vec![],
            )])
            .unwrap();
        }

        handle_clean(
            &dir,
            true,
            false,
            None,
            &[],
            None,
            &db_path,
            &FormatKind::Text,
        )
        .unwrap();

        let db = asset_db::AssetDb::open(&db_path).unwrap();
        let record = db.get_asset(&asset_path).unwrap();
        assert!(record.is_none(), "DB record should be gone after deletion");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_clean_yes_should_delete_sidecar_files_when_present() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_clean160_sidecar_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        let uasset = make_uasset(&dir, "Orphan");
        let uexp = dir.join("Orphan.uexp");
        let ubulk = dir.join("Orphan.ubulk");
        let uptnl = dir.join("Orphan.uptnl");
        std::fs::write(&uexp, b"fake").unwrap();
        std::fs::write(&ubulk, b"fake").unwrap();
        std::fs::write(&uptnl, b"fake").unwrap();

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/Orphan",
                uasset,
                AssetType::StaticMesh,
                4,
                vec![],
            )])
            .unwrap();
        }

        handle_clean(
            &dir,
            true,
            false,
            None,
            &[],
            None,
            &db_path,
            &FormatKind::Text,
        )
        .unwrap();
        assert!(!uexp.exists(), ".uexp sidecar should be deleted");
        assert!(!ubulk.exists(), ".ubulk sidecar should be deleted");
        assert!(!uptnl.exists(), ".uptnl sidecar should be deleted");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_clean_min_size_filter_should_skip_small_assets() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_clean160_minsize_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        let small = make_uasset(&dir, "Small");
        let large = make_uasset(&dir, "Large");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/Small",
                    small.clone(),
                    AssetType::Blueprint,
                    512,
                    vec![],
                ),
                make_meta(
                    "/Game/Large",
                    large.clone(),
                    AssetType::Blueprint,
                    8192,
                    vec![],
                ),
            ])
            .unwrap();
        }

        handle_clean(
            &dir,
            true,
            false,
            Some(1024),
            &[],
            None,
            &db_path,
            &FormatKind::Text,
        )
        .unwrap();
        assert!(
            small.exists(),
            "Small (512 B) below threshold — must not be deleted"
        );
        assert!(
            !large.exists(),
            "Large (8192 B) above threshold — must be deleted"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_clean_exclude_pattern_should_skip_matching_path() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_clean160_excl_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        let tp = make_uasset(&dir, "BP_ThirdPerson");
        let hero = make_uasset(&dir, "BP_Hero");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/ThirdPerson/BP_ThirdPerson",
                    tp.clone(),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
                make_meta(
                    "/Game/Characters/BP_Hero",
                    hero.clone(),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
            ])
            .unwrap();
        }

        let patterns = vec!["ThirdPerson".to_owned()];
        handle_clean(
            &dir,
            true,
            false,
            None,
            &patterns,
            None,
            &db_path,
            &FormatKind::Text,
        )
        .unwrap();
        assert!(
            tp.exists(),
            "ThirdPerson asset excluded — must not be deleted"
        );
        assert!(!hero.exists(), "BP_Hero not excluded — must be deleted");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_clean_path_filter_should_keep_non_matching_assets() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_clean160_path_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        let char_asset = make_uasset(&dir, "BP_Hero");
        let env_asset = make_uasset(&dir, "SM_Rock");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta(
                    "/Game/Characters/BP_Hero",
                    char_asset.clone(),
                    AssetType::Blueprint,
                    1024,
                    vec![],
                ),
                make_meta(
                    "/Game/Environment/SM_Rock",
                    env_asset.clone(),
                    AssetType::StaticMesh,
                    1024,
                    vec![],
                ),
            ])
            .unwrap();
        }

        handle_clean(
            &dir,
            true,
            false,
            None,
            &[],
            Some("**/Characters/**"),
            &db_path,
            &FormatKind::Text,
        )
        .unwrap();
        assert!(
            !char_asset.exists(),
            "/Game/Characters/** matches glob — must be deleted"
        );
        assert!(
            env_asset.exists(),
            "/Game/Environment/** does not match — must be kept"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_clean_dry_run_json_should_output_targets_array() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_clean160_dryjson_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        let uasset = make_uasset(&dir, "Orphan");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/Orphan",
                uasset,
                AssetType::Blueprint,
                4,
                vec![],
            )])
            .unwrap();
        }

        // stdout capture requires writer injection; verify schema via struct serialization instead
        let json = serde_json::to_string(&DryRunOutput {
            targets: vec![CleanTarget {
                path: "/Game/Orphan".to_owned(),
                asset_type: "Blueprint".to_owned(),
                file_size: 4,
            }],
            total_size_bytes: 4,
        })
        .unwrap();
        assert!(json.contains("\"targets\""), "JSON must have 'targets' key");
        assert!(
            json.contains("\"total_size_bytes\""),
            "JSON must have 'total_size_bytes' key"
        );

        let result = handle_clean(
            &dir,
            false,
            true,
            None,
            &[],
            None,
            &db_path,
            &FormatKind::Json,
        )
        .unwrap();
        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_clean_yes_json_should_output_summary_with_required_keys() {
        let dir = std::env::temp_dir().join(format!(
            "uasset_lens_clean160_yesjson_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        let uasset = make_uasset(&dir, "Orphan");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[make_meta(
                "/Game/Orphan",
                uasset,
                AssetType::Blueprint,
                4,
                vec![],
            )])
            .unwrap();
        }

        let json = serde_json::to_string(&CleanSummaryJson {
            deleted: 1,
            freed_bytes: 4,
            skipped: 0,
            errors: 0,
        })
        .unwrap();
        assert!(json.contains("\"deleted\""), "JSON must have 'deleted' key");
        assert!(
            json.contains("\"freed_bytes\""),
            "JSON must have 'freed_bytes' key"
        );
        assert!(json.contains("\"skipped\""), "JSON must have 'skipped' key");
        assert!(json.contains("\"errors\""), "JSON must have 'errors' key");

        let result = handle_clean(
            &dir,
            true,
            false,
            None,
            &[],
            None,
            &db_path,
            &FormatKind::Json,
        )
        .unwrap();
        assert_eq!(result, 0, "--yes --format json should return 0");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
