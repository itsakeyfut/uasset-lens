use std::collections::HashMap;

use asset_db::AssetRecord;
use shared::AssetPath;

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

#[cfg(test)]
mod tests {
    use super::*;
    use asset_db::AssetRecord;
    use shared::{AssetPath, AssetType};
    use std::path::PathBuf;

    fn make_record(asset_path: &str) -> AssetRecord {
        AssetRecord {
            id: 0,
            asset_path: AssetPath::new(asset_path).unwrap(),
            file_path: PathBuf::from(format!("{asset_path}.uasset")),
            asset_type: AssetType::Texture2D,
            file_size: 0,
            last_modified: 0,
        }
    }

    #[test]
    fn detect_by_name_should_return_empty_when_all_names_are_unique() {
        let assets = vec![
            make_record("/Game/Characters/T_Rock"),
            make_record("/Game/Environment/T_Grass"),
            make_record("/Game/Props/T_Wood"),
        ];
        assert!(detect_by_name(&assets).is_empty());
    }

    #[test]
    fn detect_by_name_should_return_one_group_when_two_assets_share_a_name() {
        let assets = vec![
            make_record("/Game/Characters/T_Rock"),
            make_record("/Game/Environment/T_Rock"),
        ];
        let groups = detect_by_name(&assets);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "T_Rock");
        assert_eq!(groups[0].assets.len(), 2);
    }

    #[test]
    fn detect_by_name_should_return_one_group_with_three_entries_when_three_assets_share_a_name() {
        let assets = vec![
            make_record("/Game/Characters/T_Rock"),
            make_record("/Game/Environment/T_Rock"),
            make_record("/Game/Props/T_Rock"),
        ];
        let groups = detect_by_name(&assets);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "T_Rock");
        assert_eq!(groups[0].assets.len(), 3);
    }

    #[test]
    fn detect_by_name_should_sort_assets_within_group_alphabetically() {
        let assets = vec![
            make_record("/Game/Props/T_Rock"),
            make_record("/Game/Characters/T_Rock"),
            make_record("/Game/Environment/T_Rock"),
        ];
        let groups = detect_by_name(&assets);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].assets[0].as_str(), "/Game/Characters/T_Rock");
        assert_eq!(groups[0].assets[1].as_str(), "/Game/Environment/T_Rock");
        assert_eq!(groups[0].assets[2].as_str(), "/Game/Props/T_Rock");
    }
}
