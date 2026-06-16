use std::collections::HashMap;

use uasset_lens_shared::{AssetPath, AssetType};

use crate::AssetRecord;

#[derive(Debug, Clone)]
pub struct DuplicateGroup {
    pub name: String,
    pub assets: Vec<AssetPath>,
}

pub fn detect_by_name(assets: &[AssetRecord]) -> Vec<DuplicateGroup> {
    let mut by_name: HashMap<&str, Vec<AssetPath>> = HashMap::new();
    for record in assets {
        let path = record.asset_path.as_str();
        // AssetPath always starts with '/', so rsplit always yields at least one element
        let name = path.rsplit('/').next().unwrap_or(path);
        by_name
            .entry(name)
            .or_default()
            .push(record.asset_path.clone()); // clone required: cannot move out of shared reference
    }
    by_name
        .into_iter()
        .filter(|(_, paths)| paths.len() >= 2)
        .map(|(name, mut assets)| {
            assets.sort_by(|a, b| a.as_str().cmp(b.as_str())); // deterministic order within group
            DuplicateGroup {
                name: name.to_owned(),
                assets,
            }
        })
        .collect()
}

pub fn detect_texture_duplicates(assets: &[AssetRecord]) -> Vec<DuplicateGroup> {
    let mut by_key: HashMap<(&str, u64), Vec<AssetPath>> = HashMap::new();
    for record in assets {
        if record.asset_type != AssetType::Texture2D {
            continue;
        }
        let path = record.asset_path.as_str();
        // AssetPath always starts with '/', so rsplit always yields at least one element
        let name = path.rsplit('/').next().unwrap_or(path);
        by_key
            .entry((name, record.file_size))
            .or_default()
            .push(record.asset_path.clone()); // clone required: cannot move out of shared reference
    }
    by_key
        .into_iter()
        .filter(|(_, paths)| paths.len() >= 2)
        .map(|((name, _), mut assets)| {
            assets.sort_by(|a, b| a.as_str().cmp(b.as_str())); // deterministic order within group
            DuplicateGroup {
                name: name.to_owned(),
                assets,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uasset_lens_shared::AssetType;

    #[test]
    fn detect_by_name_should_return_empty_when_all_names_are_unique() {
        let assets = vec![
            crate::make_record("/Game/Characters/T_Rock", AssetType::Texture2D),
            crate::make_record("/Game/Environment/T_Grass", AssetType::Texture2D),
            crate::make_record("/Game/Props/T_Wood", AssetType::Texture2D),
        ];
        assert!(detect_by_name(&assets).is_empty());
    }

    #[test]
    fn detect_by_name_should_return_one_group_when_two_assets_share_a_name() {
        let assets = vec![
            crate::make_record("/Game/Characters/T_Rock", AssetType::Texture2D),
            crate::make_record("/Game/Environment/T_Rock", AssetType::Texture2D),
        ];
        let groups = detect_by_name(&assets);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "T_Rock");
        assert_eq!(groups[0].assets.len(), 2);
    }

    #[test]
    fn detect_by_name_should_return_one_group_with_three_entries_when_three_assets_share_a_name() {
        let assets = vec![
            crate::make_record("/Game/Characters/T_Rock", AssetType::Texture2D),
            crate::make_record("/Game/Environment/T_Rock", AssetType::Texture2D),
            crate::make_record("/Game/Props/T_Rock", AssetType::Texture2D),
        ];
        let groups = detect_by_name(&assets);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "T_Rock");
        assert_eq!(groups[0].assets.len(), 3);
    }

    #[test]
    fn detect_texture_duplicates_should_return_one_group_when_two_textures_share_name_and_size() {
        let assets = vec![
            crate::AssetRecord {
                file_size: 4096,
                ..crate::make_record("/Game/Characters/T_Rock", AssetType::Texture2D)
            },
            crate::AssetRecord {
                file_size: 4096,
                ..crate::make_record("/Game/Environment/T_Rock", AssetType::Texture2D)
            },
        ];
        let groups = detect_texture_duplicates(&assets);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "T_Rock");
        assert_eq!(groups[0].assets.len(), 2);
    }

    #[test]
    fn detect_texture_duplicates_should_not_group_textures_with_different_sizes() {
        let assets = vec![
            crate::AssetRecord {
                file_size: 4096,
                ..crate::make_record("/Game/Characters/T_Rock", AssetType::Texture2D)
            },
            crate::AssetRecord {
                file_size: 8192,
                ..crate::make_record("/Game/Environment/T_Rock", AssetType::Texture2D)
            },
        ];
        assert!(detect_texture_duplicates(&assets).is_empty());
    }

    #[test]
    fn detect_texture_duplicates_should_not_group_textures_with_different_names() {
        let assets = vec![
            crate::AssetRecord {
                file_size: 4096,
                ..crate::make_record("/Game/Characters/T_Rock", AssetType::Texture2D)
            },
            crate::AssetRecord {
                file_size: 4096,
                ..crate::make_record("/Game/Environment/T_Grass", AssetType::Texture2D)
            },
        ];
        assert!(detect_texture_duplicates(&assets).is_empty());
    }

    #[test]
    fn detect_texture_duplicates_should_exclude_non_texture_assets_with_matching_name_and_size() {
        let assets = vec![
            crate::AssetRecord {
                file_size: 4096,
                ..crate::make_record("/Game/Characters/T_Rock", AssetType::Texture2D)
            },
            crate::AssetRecord {
                file_size: 4096,
                ..crate::make_record("/Game/Environment/T_Rock", AssetType::StaticMesh)
            },
        ];
        assert!(detect_texture_duplicates(&assets).is_empty());
    }

    #[test]
    fn detect_texture_duplicates_should_sort_assets_within_group_alphabetically() {
        let assets = vec![
            crate::AssetRecord {
                file_size: 4096,
                ..crate::make_record("/Game/Props/T_Rock", AssetType::Texture2D)
            },
            crate::AssetRecord {
                file_size: 4096,
                ..crate::make_record("/Game/Characters/T_Rock", AssetType::Texture2D)
            },
        ];
        let groups = detect_texture_duplicates(&assets);
        assert_eq!(groups[0].assets[0].as_str(), "/Game/Characters/T_Rock");
        assert_eq!(groups[0].assets[1].as_str(), "/Game/Props/T_Rock");
    }

    #[test]
    fn detect_by_name_should_sort_assets_within_group_alphabetically() {
        let assets = vec![
            crate::make_record("/Game/Props/T_Rock", AssetType::Texture2D),
            crate::make_record("/Game/Characters/T_Rock", AssetType::Texture2D),
            crate::make_record("/Game/Environment/T_Rock", AssetType::Texture2D),
        ];
        let groups = detect_by_name(&assets);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].assets[0].as_str(), "/Game/Characters/T_Rock");
        assert_eq!(groups[0].assets[1].as_str(), "/Game/Environment/T_Rock");
        assert_eq!(groups[0].assets[2].as_str(), "/Game/Props/T_Rock");
    }
}
