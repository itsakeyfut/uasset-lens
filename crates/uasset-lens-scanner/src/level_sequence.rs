use std::io::Cursor;
use uasset_lens_shared::{AssetPath, AssetType};

use crate::parser::tagged_props::scan_props_depth;

// LevelSequence track references (AnimSequence, SoundBase) are stored as
// FSoftObjectPath values in tagged property streams spread across many sub-exports
// (MovieSceneAnimationSection, MovieSceneAudioSection, etc.). Unlike AnimMontage,
// which stores everything in a single primary export, we must scan every export.

pub(crate) fn is_level_sequence_asset(asset_type: &AssetType) -> bool {
    matches!(asset_type, AssetType::LevelSequence)
}

// AnimSequence and SoundBase references inside LevelSequence live in sub-exports,
// not in the import table, so referenced assets appear dead without this extraction.
pub(crate) fn extract_level_sequence_soft_refs(
    data: &[u8],
    export_offset: u64,
    export_count: usize,
    depends_offset: u64,
    name_table: &[String],
) -> Vec<AssetPath> {
    if export_count == 0 || depends_offset <= export_offset {
        return Vec::new();
    }

    let bytes_per_entry = (depends_offset - export_offset) / export_count as u64;
    // SerialOffset is at entry_pos+36, occupies 8 bytes → need at least 44 bytes per entry.
    if bytes_per_entry < 44 {
        return Vec::new();
    }

    let mut paths = Vec::new();

    for i in 0..export_count {
        let entry_pos = (export_offset + i as u64 * bytes_per_entry) as usize;
        if entry_pos + 44 > data.len() {
            break;
        }

        let serial_size = i64::from_le_bytes([
            data[entry_pos + 28],
            data[entry_pos + 29],
            data[entry_pos + 30],
            data[entry_pos + 31],
            data[entry_pos + 32],
            data[entry_pos + 33],
            data[entry_pos + 34],
            data[entry_pos + 35],
        ]);
        let serial_offset = i64::from_le_bytes([
            data[entry_pos + 36],
            data[entry_pos + 37],
            data[entry_pos + 38],
            data[entry_pos + 39],
            data[entry_pos + 40],
            data[entry_pos + 41],
            data[entry_pos + 42],
            data[entry_pos + 43],
        ]);

        // Skip empty/invalid exports; continue to next rather than aborting.
        if serial_size <= 0 || serial_offset <= 0 {
            continue;
        }

        let start = serial_offset as usize;
        let end = start.saturating_add(serial_size as usize);
        if end > data.len() {
            continue;
        }

        let mut cur = Cursor::new(&data[start..end]);
        let _ = scan_props_depth(&mut cur, name_table, &mut paths, 0);
    }

    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fname(idx: i32) -> Vec<u8> {
        let mut v = idx.to_le_bytes().to_vec();
        v.extend_from_slice(&0i32.to_le_bytes());
        v
    }

    fn i32_le(v: i32) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }

    fn i64_le(v: i64) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }

    // name_table indices:
    //  0  = "None"
    //  1  = "LevelSequence"
    //  2  = "Animation"
    //  3  = "SoftObjectProperty"
    //  4  = "/Game/Anims/AS_Run"
    //  5  = "Sound"
    //  6  = "/Game/Sounds/SW_Fire"
    //  7  = "ArrayProperty"
    //  8  = "StructProperty"
    //  9  = "FMovieSceneAnimationSectionData"
    // 10  = "/Game/Anims/AS_Jump"
    fn name_table() -> Vec<String> {
        vec![
            "None".into(),
            "LevelSequence".into(),
            "Animation".into(),
            "SoftObjectProperty".into(),
            "/Game/Anims/AS_Run".into(),
            "Sound".into(),
            "/Game/Sounds/SW_Fire".into(),
            "ArrayProperty".into(),
            "StructProperty".into(),
            "FMovieSceneAnimationSectionData".into(),
            "/Game/Anims/AS_Jump".into(),
        ]
    }

    fn none_terminator() -> Vec<u8> {
        fname(0) // "None"
    }

    fn soft_object_prop(name_idx: i32, pkg_idx: i32) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend(fname(name_idx)); // prop name
        v.extend(fname(3)); // type: "SoftObjectProperty"
        v.extend(i64_le(20)); // prop_size: FName(8)+FName(8)+FString(4)
        v.extend(i32_le(0)); // array_idx
        v.extend(fname(pkg_idx)); // PackageName FName
        v.extend(fname(0)); // AssetName FName
        v.extend(i32_le(0)); // SubPathString len = 0
        v
    }

    // Builds (data, export_count, depends_offset) from a list of serial data blocks.
    // Each serial block occupies one export entry in the table.
    fn wrap_with_exports(serials: Vec<Vec<u8>>) -> (Vec<u8>, usize, u64) {
        let bytes_per_entry: usize = 112;
        let export_count = serials.len();
        let table_size = bytes_per_entry * export_count;

        let mut data: Vec<u8> = vec![0u8; table_size];
        let mut current_offset = table_size;

        for (i, serial) in serials.iter().enumerate() {
            let entry_pos = i * bytes_per_entry;
            let serial_size = serial.len() as i64;
            let serial_offset = current_offset as i64;
            data[entry_pos..entry_pos + 4].copy_from_slice(&(-1i32).to_le_bytes());
            data[entry_pos + 28..entry_pos + 36].copy_from_slice(&serial_size.to_le_bytes());
            data[entry_pos + 36..entry_pos + 44].copy_from_slice(&serial_offset.to_le_bytes());
            current_offset += serial.len();
        }

        for serial in serials {
            data.extend(serial);
        }

        let depends_offset = table_size as u64;
        (data, export_count, depends_offset)
    }

    #[test]
    fn is_level_sequence_asset_should_return_true_for_level_sequence() {
        assert!(is_level_sequence_asset(&AssetType::LevelSequence));
    }

    #[test]
    fn is_level_sequence_asset_should_return_false_for_non_level_sequence() {
        assert!(!is_level_sequence_asset(&AssetType::AnimMontage));
        assert!(!is_level_sequence_asset(&AssetType::AnimSequence));
        assert!(!is_level_sequence_asset(&AssetType::Blueprint));
    }

    #[test]
    fn extract_level_sequence_soft_refs_should_return_empty_for_zero_export_count() {
        let result = extract_level_sequence_soft_refs(&[], 0, 0, 0, &name_table());
        assert!(result.is_empty());
    }

    #[test]
    fn extract_level_sequence_soft_refs_should_extract_soft_ref_from_single_export() {
        let mut serial = soft_object_prop(2, 4); // "Animation" → /Game/Anims/AS_Run
        serial.extend(none_terminator());

        let (data, export_count, depends_offset) = wrap_with_exports(vec![serial]);
        let result =
            extract_level_sequence_soft_refs(&data, 0, export_count, depends_offset, &name_table());

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_str(), "/Game/Anims/AS_Run");
    }

    #[test]
    fn extract_level_sequence_soft_refs_should_extract_refs_from_multiple_exports() {
        // AnimSequence ref in first sub-export, SoundBase ref in second.
        let mut anim_serial = soft_object_prop(2, 4); // "Animation" → AS_Run
        anim_serial.extend(none_terminator());

        let mut sound_serial = soft_object_prop(5, 6); // "Sound" → SW_Fire
        sound_serial.extend(none_terminator());

        let (data, export_count, depends_offset) =
            wrap_with_exports(vec![anim_serial, sound_serial]);
        let result =
            extract_level_sequence_soft_refs(&data, 0, export_count, depends_offset, &name_table());

        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|p| p.as_str() == "/Game/Anims/AS_Run"));
        assert!(result.iter().any(|p| p.as_str() == "/Game/Sounds/SW_Fire"));
    }

    #[test]
    fn extract_level_sequence_soft_refs_should_skip_exports_with_zero_serial_size() {
        // First export has zero serial_size (e.g., a generated export with no body).
        // Second export has a valid soft ref. Both must survive; only refs from the
        // second export appear.
        let bytes_per_entry: usize = 112;
        let export_count: usize = 2;
        let table_size = bytes_per_entry * export_count;

        let mut sound_serial = soft_object_prop(5, 6); // SW_Fire
        sound_serial.extend(none_terminator());

        let mut data: Vec<u8> = vec![0u8; table_size];
        // Entry 0: serial_size = 0 → skip
        data[0..4].copy_from_slice(&(-1i32).to_le_bytes());
        // serial_size at offset 28 stays 0
        // Entry 1: valid serial
        let entry1 = bytes_per_entry;
        let serial_offset = table_size as i64;
        let serial_size = sound_serial.len() as i64;
        data[entry1..entry1 + 4].copy_from_slice(&(-1i32).to_le_bytes());
        data[entry1 + 28..entry1 + 36].copy_from_slice(&serial_size.to_le_bytes());
        data[entry1 + 36..entry1 + 44].copy_from_slice(&serial_offset.to_le_bytes());
        data.extend(sound_serial);

        let depends_offset = table_size as u64;
        let result =
            extract_level_sequence_soft_refs(&data, 0, export_count, depends_offset, &name_table());

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_str(), "/Game/Sounds/SW_Fire");
    }

    #[test]
    fn extract_level_sequence_soft_refs_should_skip_non_game_paths() {
        let mut serial = soft_object_prop(2, 1); // pkg = "LevelSequence" (not /Game/)
        serial.extend(none_terminator());

        let (data, export_count, depends_offset) = wrap_with_exports(vec![serial]);
        let result =
            extract_level_sequence_soft_refs(&data, 0, export_count, depends_offset, &name_table());

        assert!(result.is_empty());
    }

    #[test]
    fn extract_level_sequence_soft_refs_should_extract_ref_from_nested_struct() {
        // Animation property nested inside a StructProperty element.
        let mut inner = soft_object_prop(2, 10); // AS_Jump
        inner.extend(none_terminator());

        // StructProperty wrapper
        let body_size = inner.len() as i64;
        let mut serial = Vec::new();
        serial.extend(fname(9)); // "FMovieSceneAnimationSectionData" as prop name
        serial.extend(fname(8)); // type: "StructProperty"
        serial.extend(i64_le(body_size));
        serial.extend(i32_le(0)); // array_idx
        serial.extend(fname(9)); // struct name FName
        serial.extend(vec![0u8; 16]); // FGuid
        serial.extend(inner);
        serial.extend(none_terminator());

        let (data, export_count, depends_offset) = wrap_with_exports(vec![serial]);
        let result =
            extract_level_sequence_soft_refs(&data, 0, export_count, depends_offset, &name_table());

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_str(), "/Game/Anims/AS_Jump");
    }
}
