use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Context;

use crate::FormatKind;

#[derive(serde::Serialize)]
struct DuplicateAsset {
    path: String,
    #[serde(rename = "type")]
    asset_type: String,
}

#[derive(serde::Serialize)]
struct DuplicateEntry {
    #[serde(rename = "type")]
    kind: String,
    name: String,
    /// The asset type shared by every copy, or `None` when the copies have differing types.
    shared_type: Option<String>,
    assets: Vec<DuplicateAsset>,
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

    let by_name_groups = uasset_lens_asset_db::duplicates::detect_by_name(&assets);
    let texture_dup_groups = uasset_lens_asset_db::duplicates::detect_texture_duplicates(&assets);

    // Texture-dup is more specific (same name + size + type); suppress the same group
    // from appearing again under same-name.
    let texture_dup_names: HashSet<&str> =
        texture_dup_groups.iter().map(|g| g.name.as_str()).collect();

    let type_by_path: HashMap<&str, String> = assets
        .iter()
        .map(|r| (r.asset_path.as_str(), r.asset_type.to_string()))
        .collect();

    let make_entry = |kind: &str, g: &uasset_lens_asset_db::duplicates::DuplicateGroup| {
        let dup_assets = build_dup_assets(&g.assets, &type_by_path);
        DuplicateEntry {
            kind: kind.to_owned(),
            name: g.name.clone(), // clone required: DuplicateEntry owns the name
            shared_type: compute_shared_type(&dup_assets),
            assets: dup_assets,
        }
    };

    let mut entries: Vec<DuplicateEntry> = texture_dup_groups
        .iter()
        .map(|g| make_entry("texture-dup", g))
        .chain(
            by_name_groups
                .iter()
                .filter(|g| !texture_dup_names.contains(g.name.as_str()))
                .map(|g| make_entry("same-name", g)),
        )
        .collect();

    entries.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.name.cmp(&b.name)));

    let has_duplicates = !entries.is_empty();

    match format {
        FormatKind::Sarif => return Err(crate::sarif_not_supported()),
        FormatKind::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&entries)
                    .context("Failed to serialize duplicates output to JSON")?
            );
        }
        FormatKind::GithubActions | FormatKind::Text => {
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
                            .and_then(|a| asset_size_map.get(a.path.as_str()))
                            .copied()
                            .unwrap_or(0);
                        println!(
                            "[Texture duplicate] {} ({} \u{d7} {})",
                            entry.name,
                            crate::format_size(file_size),
                            entry.assets.len(),
                        );
                    } else {
                        let suffix = match &entry.shared_type {
                            Some(t) => format!(", {t}"),
                            None => format!(", mixed: {}", mixed_types(&entry.assets)),
                        };
                        println!(
                            "[Same name{}] {} ({} copies)",
                            suffix,
                            entry.name,
                            entry.assets.len(),
                        );
                    }
                    // When the copies share a type it is already in the header; only spell out
                    // the per-path type for mixed-type groups.
                    for asset in &entry.assets {
                        match &entry.shared_type {
                            Some(_) => println!("  {}", asset.path),
                            None => println!("  {}  ({})", asset.path, asset.asset_type),
                        }
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

/// Resolves each duplicate path to its asset type (empty string if the path is missing from the
/// type map, which should not happen — every duplicate path is a scanned asset).
fn build_dup_assets(
    paths: &[uasset_lens_shared::AssetPath],
    type_by_path: &HashMap<&str, String>,
) -> Vec<DuplicateAsset> {
    paths
        .iter()
        .map(|p| DuplicateAsset {
            path: p.as_str().to_owned(),
            asset_type: type_by_path.get(p.as_str()).cloned().unwrap_or_default(),
        })
        .collect()
}

/// The asset type shared by every copy, or `None` when they differ (a "mixed" group).
fn compute_shared_type(assets: &[DuplicateAsset]) -> Option<String> {
    let first = assets.first()?.asset_type.as_str();
    assets
        .iter()
        .all(|a| a.asset_type == first)
        .then(|| first.to_owned())
}

/// The distinct asset types of a mixed group, sorted and joined for the header (e.g.
/// `Material / MaterialInstance`).
fn mixed_types(assets: &[DuplicateAsset]) -> String {
    let mut types: Vec<&str> = assets.iter().map(|a| a.asset_type.as_str()).collect();
    types.sort_unstable();
    types.dedup();
    types.join(" / ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_db_in_tempdir;
    use uasset_lens_shared::AssetType;

    fn dup(path: &str, ty: &str) -> DuplicateAsset {
        DuplicateAsset {
            path: path.to_owned(),
            asset_type: ty.to_owned(),
        }
    }

    #[test]
    fn compute_shared_type_should_be_some_when_all_copies_share_a_type() {
        let assets = vec![
            dup("/Game/A", "SkeletalMesh"),
            dup("/Game/B", "SkeletalMesh"),
        ];
        assert_eq!(
            compute_shared_type(&assets),
            Some("SkeletalMesh".to_owned())
        );
    }

    #[test]
    fn compute_shared_type_should_be_none_when_copies_have_mixed_types() {
        let assets = vec![
            dup("/Game/A", "Material"),
            dup("/Game/B", "MaterialInstance"),
        ];
        assert_eq!(compute_shared_type(&assets), None);
    }

    #[test]
    fn mixed_types_should_sort_and_dedup_distinct_types() {
        let assets = vec![
            dup("/Game/A", "MaterialInstance"),
            dup("/Game/B", "Material"),
            dup("/Game/C", "Material"),
        ];
        assert_eq!(mixed_types(&assets), "Material / MaterialInstance");
    }

    #[test]
    fn duplicate_entry_json_should_include_shared_type_and_per_asset_type() {
        let entry = DuplicateEntry {
            kind: "same-name".to_owned(),
            name: "M_Flare".to_owned(),
            shared_type: None,
            assets: vec![
                dup("/Game/A", "Material"),
                dup("/Game/B", "MaterialInstance"),
            ],
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(
            json.contains("\"shared_type\":null"),
            "mixed group must serialize shared_type as null"
        );
        assert!(
            json.contains("\"path\":\"/Game/A\""),
            "each asset must carry its path"
        );
        assert!(
            json.contains("\"type\":\"Material\""),
            "each asset must carry its own type"
        );
    }

    #[test]
    fn handle_duplicates_should_return_err_when_db_does_not_exist() {
        let (dir, db_path) = test_db_in_tempdir("dupes_missing");
        let _ = std::fs::remove_dir_all(&dir);
        let result = handle_duplicates(std::path::Path::new("."), &db_path, &FormatKind::Text);
        assert!(result.is_err());
    }

    #[test]
    fn handle_duplicates_should_return_0_when_no_assets_in_db() {
        let (dir, db_path) = test_db_in_tempdir("dupes_empty");
        uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();

        let result = handle_duplicates(&dir, &db_path, &FormatKind::Text).unwrap();

        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_duplicates_should_return_0_when_all_assets_are_unique() {
        let (dir, db_path) = test_db_in_tempdir("dupes_unique");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                uasset_lens_scanner::AssetMetadata {
                    file_size: 1024,
                    ..uasset_lens_scanner::make_meta("/Game/A/T_Rock", AssetType::Texture2D)
                },
                uasset_lens_scanner::AssetMetadata {
                    file_size: 2048,
                    ..uasset_lens_scanner::make_meta("/Game/B/T_Grass", AssetType::Texture2D)
                },
            ])
            .unwrap();
        }

        let result = handle_duplicates(&dir, &db_path, &FormatKind::Text).unwrap();

        assert_eq!(result, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_duplicates_should_return_1_when_same_name_duplicates_found() {
        let (dir, db_path) = test_db_in_tempdir("dupes_name");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                uasset_lens_scanner::AssetMetadata {
                    file_size: 1024,
                    ..uasset_lens_scanner::make_meta(
                        "/Game/Characters/T_Rock",
                        AssetType::Texture2D,
                    )
                },
                uasset_lens_scanner::AssetMetadata {
                    file_size: 2048,
                    ..uasset_lens_scanner::make_meta(
                        "/Game/Environment/T_Rock",
                        AssetType::Texture2D,
                    )
                },
            ])
            .unwrap();
        }

        let result = handle_duplicates(&dir, &db_path, &FormatKind::Text).unwrap();

        assert_eq!(result, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_duplicates_should_return_1_when_texture_duplicates_found() {
        let (dir, db_path) = test_db_in_tempdir("dupes_tex");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                uasset_lens_scanner::AssetMetadata {
                    file_size: 4096,
                    ..uasset_lens_scanner::make_meta(
                        "/Game/Characters/T_Rock",
                        AssetType::Texture2D,
                    )
                },
                uasset_lens_scanner::AssetMetadata {
                    file_size: 4096,
                    ..uasset_lens_scanner::make_meta(
                        "/Game/Environment/T_Rock",
                        AssetType::Texture2D,
                    )
                },
            ])
            .unwrap();
        }

        let result = handle_duplicates(&dir, &db_path, &FormatKind::Text).unwrap();

        assert_eq!(result, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_duplicates_json_should_return_1_when_duplicates_found() {
        let (dir, db_path) = test_db_in_tempdir("dupes_json");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            db.upsert_all(&[
                uasset_lens_scanner::AssetMetadata {
                    file_size: 1024,
                    ..uasset_lens_scanner::make_meta(
                        "/Game/Characters/T_Rock",
                        AssetType::Texture2D,
                    )
                },
                uasset_lens_scanner::AssetMetadata {
                    file_size: 1024,
                    ..uasset_lens_scanner::make_meta(
                        "/Game/Environment/T_Rock",
                        AssetType::Texture2D,
                    )
                },
            ])
            .unwrap();
        }

        let result = handle_duplicates(&dir, &db_path, &FormatKind::Json).unwrap();

        assert_eq!(result, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn handle_duplicates_should_label_texture_dup_not_same_name_when_same_name_and_size() {
        let (dir, db_path) = test_db_in_tempdir("dupes_dedup");

        {
            let mut db = uasset_lens_asset_db::AssetDb::open(&db_path).unwrap();
            // Same name AND same size → should appear as texture-dup only, not also same-name
            db.upsert_all(&[
                uasset_lens_scanner::AssetMetadata {
                    file_size: 4096,
                    ..uasset_lens_scanner::make_meta(
                        "/Game/Characters/T_Rock",
                        AssetType::Texture2D,
                    )
                },
                uasset_lens_scanner::AssetMetadata {
                    file_size: 4096,
                    ..uasset_lens_scanner::make_meta(
                        "/Game/Environment/T_Rock",
                        AssetType::Texture2D,
                    )
                },
            ])
            .unwrap();
        }

        let db = uasset_lens_asset_db::AssetDb::open_existing(&db_path).unwrap();
        let assets = db.all_assets().unwrap();
        let by_name = uasset_lens_asset_db::duplicates::detect_by_name(&assets);
        let tex_dup = uasset_lens_asset_db::duplicates::detect_texture_duplicates(&assets);

        // Both detectors fire for T_Rock
        assert_eq!(by_name.len(), 1);
        assert_eq!(tex_dup.len(), 1);

        // The CLI handler should emit only 1 group (texture-dup, not same-name)
        let result = handle_duplicates(&dir, &db_path, &FormatKind::Text).unwrap();
        assert_eq!(result, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
