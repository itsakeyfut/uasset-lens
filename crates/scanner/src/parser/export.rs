use shared::AssetType;

use crate::ScanError;

// FObjectExport layout (UE5, empirically verified):
//   ClassIndex    (i32) = 4 bytes  — FPackageIndex: negative → import[abs-1], 0 → UObject, positive → local export
//   SuperIndex    (i32) = 4 bytes
//   TemplateIndex (i32) = 4 bytes
//   OuterIndex    (i32) = 4 bytes
//   ObjectName    (FName: i32 index + i32 number) = 8 bytes
//   ObjectFlags   (u32)  = 4 bytes
//   SerialSize    (i64)  = 8 bytes
//   SerialOffset  (i64)  = 8 bytes
//   bForcedExport (serialised as i32) = 4 bytes
//   bNotForClient (serialised as i32) = 4 bytes
//   bNotForServer (serialised as i32) = 4 bytes
//   PackageGuid   (FGuid) = 16 bytes
//   PackageFlags  (u32)  = 4 bytes
//   [3 booleans added in a later UE5 revision — bNotAlwaysLoadedForEditorGame, bIsAsset, bGeneratePublicHash]
//   FirstExportDependency + 4 dependency counts (5 × i32) = 20 bytes
//
// Entry size varies by UE5 file version: 96 bytes (older) or 112 bytes (newer).
// Computed at runtime as (depends_offset - export_offset) / export_count.
//
// AssetType detection: iterate exports in order, resolve ClassIndex to the import
// entry's ObjectName, return on the first match from the known-types table.
pub fn parse_export_table(
    data: &[u8],
    offset: u64,
    count: usize,
    depends_offset: u64,
    import_class_name_idxs: &[usize],
    name_table: &[String],
) -> Result<AssetType, ScanError> {
    if count == 0 || depends_offset <= offset {
        return Ok(AssetType::Unknown(String::new()));
    }

    let bytes_per_entry = (depends_offset - offset) / count as u64;
    if bytes_per_entry < 4 {
        return Ok(AssetType::Unknown(String::new()));
    }

    let mut first_class_name = None;

    for i in 0..count {
        let entry_pos = (offset + i as u64 * bytes_per_entry) as usize;
        if entry_pos + 4 > data.len() {
            return Err(ScanError::UnexpectedEof);
        }
        let class_index = i32::from_le_bytes([
            data[entry_pos],
            data[entry_pos + 1],
            data[entry_pos + 2],
            data[entry_pos + 3],
        ]);

        if class_index >= 0 {
            continue;
        }

        let imp_i = (-class_index - 1) as usize;
        if let Some(class_name) = import_class_name_idxs
            .get(imp_i)
            .and_then(|&idx| name_table.get(idx))
        {
            if first_class_name.is_none() {
                first_class_name = Some(class_name.clone());
            }
            if let Some(asset_type) = class_name_to_asset_type(class_name) {
                return Ok(asset_type);
            }
        }
    }

    Ok(AssetType::Unknown(first_class_name.unwrap_or_default()))
}

fn class_name_to_asset_type(name: &str) -> Option<AssetType> {
    match name {
        "Blueprint" => Some(AssetType::Blueprint),
        "BlueprintInterface" => Some(AssetType::BlueprintInterface),
        "AnimBlueprint" => Some(AssetType::AnimBlueprint),
        "WidgetBlueprint" => Some(AssetType::UserWidget),
        "Texture2D" => Some(AssetType::Texture2D),
        "StaticMesh" => Some(AssetType::StaticMesh),
        "SkeletalMesh" => Some(AssetType::SkeletalMesh),
        "Material" => Some(AssetType::Material),
        "MaterialInstanceConstant" => Some(AssetType::MaterialInstance),
        "MaterialFunction" => Some(AssetType::MaterialFunction),
        "SoundWave" => Some(AssetType::SoundWave),
        "SoundCue" => Some(AssetType::SoundCue),
        "AnimSequence" => Some(AssetType::AnimSequence),
        "AnimMontage" => Some(AssetType::AnimMontage),
        "LevelSequence" => Some(AssetType::LevelSequence),
        "DataTable" => Some(AssetType::DataTable),
        "DataAsset" => Some(AssetType::DataAsset),
        "ObjectRedirector" => Some(AssetType::ObjectRedirector),
        "NiagaraSystem" => Some(AssetType::NiagaraSystem),
        "NiagaraEmitter" => Some(AssetType::NiagaraEmitter),
        "IKRigDefinition" => Some(AssetType::IKRigDefinition),
        "IKRetargeter" => Some(AssetType::IKRetargeter),
        "DialogueWave" => Some(AssetType::DialogueWave),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_utils::*;
    use super::*;
    use crate::parser::{
        header::parse_header, import::parse_import_entries, name_table::parse_name_table,
    };

    #[test]
    fn parse_export_table_should_return_blueprint_for_bp_simple_fixture() {
        let data = read_fixture("valid/BP_Simple.uasset");
        let hdr = parse_header(&data).unwrap();
        let names = parse_name_table(&data, hdr.name_offset, hdr.name_count).unwrap();
        let (cls_idxs, _) =
            parse_import_entries(&data, hdr.import_offset, hdr.import_count, &names).unwrap();
        let result = parse_export_table(
            &data,
            hdr.export_offset,
            hdr.export_count,
            hdr.depends_offset,
            &cls_idxs,
            &names,
        )
        .unwrap();
        assert_eq!(result, AssetType::Blueprint);
    }

    #[test]
    fn parse_export_table_should_return_texture2d_for_t_rock_fixture() {
        let data = read_fixture("valid/T_Rock.uasset");
        let hdr = parse_header(&data).unwrap();
        let names = parse_name_table(&data, hdr.name_offset, hdr.name_count).unwrap();
        let (cls_idxs, _) =
            parse_import_entries(&data, hdr.import_offset, hdr.import_count, &names).unwrap();
        let result = parse_export_table(
            &data,
            hdr.export_offset,
            hdr.export_count,
            hdr.depends_offset,
            &cls_idxs,
            &names,
        )
        .unwrap();
        assert_eq!(result, AssetType::Texture2D);
    }

    #[test]
    fn parse_export_table_should_return_object_redirector_for_redirect_fixture() {
        let data = read_fixture("valid/Redirect.uasset");
        let hdr = parse_header(&data).unwrap();
        let names = parse_name_table(&data, hdr.name_offset, hdr.name_count).unwrap();
        let (cls_idxs, _) =
            parse_import_entries(&data, hdr.import_offset, hdr.import_count, &names).unwrap();
        let result = parse_export_table(
            &data,
            hdr.export_offset,
            hdr.export_count,
            hdr.depends_offset,
            &cls_idxs,
            &names,
        )
        .unwrap();
        assert_eq!(result, AssetType::ObjectRedirector);
    }

    #[test]
    fn parse_export_table_should_return_unknown_for_empty_count() {
        let data = vec![0u8; 64];
        let result = parse_export_table(&data, 0, 0, 0, &[], &[]).unwrap();
        assert!(matches!(result, AssetType::Unknown(_)));
    }

    #[test]
    fn class_name_to_asset_type_should_return_blueprint_interface_for_blueprint_interface() {
        assert_eq!(
            class_name_to_asset_type("BlueprintInterface"),
            Some(AssetType::BlueprintInterface)
        );
    }

    #[test]
    fn class_name_to_asset_type_should_return_material_function_for_material_function() {
        assert_eq!(
            class_name_to_asset_type("MaterialFunction"),
            Some(AssetType::MaterialFunction)
        );
    }

    #[test]
    fn class_name_to_asset_type_should_return_sound_wave_for_sound_wave() {
        assert_eq!(
            class_name_to_asset_type("SoundWave"),
            Some(AssetType::SoundWave)
        );
    }

    #[test]
    fn class_name_to_asset_type_should_return_sound_cue_for_sound_cue() {
        assert_eq!(
            class_name_to_asset_type("SoundCue"),
            Some(AssetType::SoundCue)
        );
    }

    #[test]
    fn class_name_to_asset_type_should_return_anim_sequence_for_anim_sequence() {
        assert_eq!(
            class_name_to_asset_type("AnimSequence"),
            Some(AssetType::AnimSequence)
        );
    }

    #[test]
    fn class_name_to_asset_type_should_return_anim_montage_for_anim_montage() {
        assert_eq!(
            class_name_to_asset_type("AnimMontage"),
            Some(AssetType::AnimMontage)
        );
    }

    #[test]
    fn class_name_to_asset_type_should_return_level_sequence_for_level_sequence() {
        assert_eq!(
            class_name_to_asset_type("LevelSequence"),
            Some(AssetType::LevelSequence)
        );
    }

    #[test]
    fn class_name_to_asset_type_should_return_data_table_for_data_table() {
        assert_eq!(
            class_name_to_asset_type("DataTable"),
            Some(AssetType::DataTable)
        );
    }

    #[test]
    fn class_name_to_asset_type_should_return_data_asset_for_data_asset() {
        assert_eq!(
            class_name_to_asset_type("DataAsset"),
            Some(AssetType::DataAsset)
        );
    }

    #[test]
    fn class_name_to_asset_type_should_return_niagara_system_for_niagara_system() {
        assert_eq!(
            class_name_to_asset_type("NiagaraSystem"),
            Some(AssetType::NiagaraSystem)
        );
    }

    #[test]
    fn class_name_to_asset_type_should_return_ik_rig_definition_for_ik_rig_definition() {
        assert_eq!(
            class_name_to_asset_type("IKRigDefinition"),
            Some(AssetType::IKRigDefinition)
        );
    }

    #[test]
    fn class_name_to_asset_type_should_return_none_for_unknown_class() {
        assert_eq!(class_name_to_asset_type(""), None);
        assert_eq!(class_name_to_asset_type("SomeCustomClass"), None);
    }
}
