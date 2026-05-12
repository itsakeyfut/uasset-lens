use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Context;

use crate::FormatKind;

#[derive(serde::Serialize)]
struct DuplicateEntry {
    #[serde(rename = "type")]
    kind: String,
    name: String,
    assets: Vec<String>,
}

pub fn handle_duplicates(
    _project_dir: &Path,
    db_path: &Path,
    format: &FormatKind,
) -> anyhow::Result<i32> {
    let db = crate::open_db(db_path)?;

    let assets = db
        .all_assets()
        .context("Failed to read assets from database")?;

    let by_name_groups = duplicate_detector::detect_by_name(&assets);
    let texture_dup_groups = duplicate_detector::detect_texture_duplicates(&assets);

    // Texture-dup is more specific (same name + size + type); suppress the same group
    // from appearing again under same-name.
    let texture_dup_names: HashSet<&str> =
        texture_dup_groups.iter().map(|g| g.name.as_str()).collect();

    let mut entries: Vec<DuplicateEntry> = texture_dup_groups
        .iter()
        .map(|g| DuplicateEntry {
            kind: "texture-dup".to_owned(),
            name: g.name.clone(), // clone required: DuplicateEntry owns the name
            assets: g.assets.iter().map(|p| p.as_str().to_owned()).collect(),
        })
        .chain(
            by_name_groups
                .iter()
                .filter(|g| !texture_dup_names.contains(g.name.as_str()))
                .map(|g| DuplicateEntry {
                    kind: "same-name".to_owned(),
                    name: g.name.clone(), // clone required: DuplicateEntry owns the name
                    assets: g.assets.iter().map(|p| p.as_str().to_owned()).collect(),
                }),
        )
        .collect();

    entries.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.name.cmp(&b.name)));

    let has_duplicates = !entries.is_empty();

    match format {
        FormatKind::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&entries)
                    .context("Failed to serialize duplicates output to JSON")?
            );
        }
        FormatKind::Text => {
            println!("Duplicate Assets");
            println!("================");
            if entries.is_empty() {
                println!("  (no duplicate assets found)");
            } else {
                let asset_size_map: HashMap<&str, u64> = assets
                    .iter()
                    .map(|r| (r.asset_path.as_str(), r.file_size))
                    .collect();
                for entry in &entries {
                    if entry.kind == "texture-dup" {
                        let file_size = entry
                            .assets
                            .first()
                            .and_then(|p| asset_size_map.get(p.as_str()))
                            .copied()
                            .unwrap_or(0);
                        println!(
                            "[Texture duplicate] {} ({} \u{d7} {})",
                            entry.name,
                            crate::format_size(file_size),
                            entry.assets.len(),
                        );
                    } else {
                        println!("[Same name] {} ({} copies)", entry.name, entry.assets.len(),);
                    }
                    for path in &entry.assets {
                        println!("  {}", path);
                    }
                    println!();
                }
                let n = entries.len();
                println!(
                    "{} duplicate {} found.",
                    n,
                    if n == 1 { "group" } else { "groups" }
                );
            }
        }
    }

    if has_duplicates { Ok(1) } else { Ok(0) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::AssetType;

    fn make_meta(
        asset_path: &str,
        asset_type: AssetType,
        file_size: u64,
    ) -> scanner::AssetMetadata {
        scanner::AssetMetadata {
            asset_path: shared::AssetPath::new(asset_path).unwrap(),
            file_path: std::path::PathBuf::from(format!("{asset_path}.uasset")),
            asset_type,
            file_size,
            last_modified: 0,
            dependencies: vec![],
            blueprint_metrics: None,
            material_texture_samples: None,
        }
    }

    #[test]
    fn handle_duplicates_should_return_err_when_db_does_not_exist() {
        let path = std::env::temp_dir().join(format!(
            "uasset_lens_dupes_missing_{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let result = handle_duplicates(std::path::Path::new("."), &path, &FormatKind::Text);
        assert!(result.is_err());
    }

    #[test]
    fn handle_duplicates_should_return_0_when_no_assets_in_db() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_dupes_empty_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");
        asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_duplicates(&dir, &db_path, &FormatKind::Text).unwrap();

        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_duplicates_should_return_0_when_all_assets_are_unique() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_dupes_unique_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta("/Game/A/T_Rock", AssetType::Texture2D, 1024),
                make_meta("/Game/B/T_Grass", AssetType::Texture2D, 2048),
            ])
            .unwrap();
        }

        let result = handle_duplicates(&dir, &db_path, &FormatKind::Text).unwrap();

        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_duplicates_should_return_1_when_same_name_duplicates_found() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_dupes_name_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta("/Game/Characters/T_Rock", AssetType::Texture2D, 1024),
                make_meta("/Game/Environment/T_Rock", AssetType::Texture2D, 2048),
            ])
            .unwrap();
        }

        let result = handle_duplicates(&dir, &db_path, &FormatKind::Text).unwrap();

        assert_eq!(result, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_duplicates_should_return_1_when_texture_duplicates_found() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_dupes_tex_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta("/Game/Characters/T_Rock", AssetType::Texture2D, 4096),
                make_meta("/Game/Environment/T_Rock", AssetType::Texture2D, 4096),
            ])
            .unwrap();
        }

        let result = handle_duplicates(&dir, &db_path, &FormatKind::Text).unwrap();

        assert_eq!(result, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_duplicates_json_should_return_1_when_duplicates_found() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_dupes_json_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                make_meta("/Game/Characters/T_Rock", AssetType::Texture2D, 1024),
                make_meta("/Game/Environment/T_Rock", AssetType::Texture2D, 1024),
            ])
            .unwrap();
        }

        let result = handle_duplicates(&dir, &db_path, &FormatKind::Json).unwrap();

        assert_eq!(result, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_duplicates_should_label_texture_dup_not_same_name_when_same_name_and_size() {
        let dir =
            std::env::temp_dir().join(format!("uasset_lens_dupes_dedup_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        {
            let mut db = asset_db::AssetDb::open(&db_path).unwrap();
            // Same name AND same size → should appear as texture-dup only, not also same-name
            db.upsert_all(&[
                make_meta("/Game/Characters/T_Rock", AssetType::Texture2D, 4096),
                make_meta("/Game/Environment/T_Rock", AssetType::Texture2D, 4096),
            ])
            .unwrap();
        }

        let db = asset_db::AssetDb::open_existing(&db_path).unwrap();
        let assets = db.all_assets().unwrap();
        let by_name = duplicate_detector::detect_by_name(&assets);
        let tex_dup = duplicate_detector::detect_texture_duplicates(&assets);

        // Both detectors fire for T_Rock
        assert_eq!(by_name.len(), 1);
        assert_eq!(tex_dup.len(), 1);

        // The CLI handler should emit only 1 group (texture-dup, not same-name)
        let result = handle_duplicates(&dir, &db_path, &FormatKind::Text).unwrap();
        assert_eq!(result, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
