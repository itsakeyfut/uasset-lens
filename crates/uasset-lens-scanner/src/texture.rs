use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;
use uasset_lens_shared::AssetType;

use crate::ScanError;
use crate::parser::{advance, export_class_name, map_io};

/// Texture object classes that carry the metadata extracted here. UE5 serializes
/// `CompressionSettings`, `MipGenSettings`, and `LODGroup` identically across all four.
const TEXTURE_CLASSES: [&str; 4] = [
    "Texture2D",
    "TextureCube",
    "Texture2DArray",
    "VolumeTexture",
];

/// Texture metadata read from the export property stream. Fields default to `None` when the
/// property is absent from the binary — UE omits properties left at their class-default value.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TextureMetadata {
    pub compression: Option<String>,
    pub mip_gen: Option<String>,
    pub texture_group: Option<String>,
}

pub(crate) fn is_texture_asset(asset_type: &AssetType) -> bool {
    match asset_type {
        AssetType::Texture2D => true,
        AssetType::Unknown(s) => TEXTURE_CLASSES.contains(&s.as_str()),
        _ => false,
    }
}

// Reads CompressionSettings / MipGenSettings / LODGroup from the first texture export's
// tagged-property stream. These are serialized as ByteProperty enum values whose names
// (e.g. "TC_Default", "TEXTUREGROUP_World") live in the name table. Any parse error is
// non-fatal: whatever was captured before the error is returned, the rest stays None.
pub(crate) fn extract_texture_metadata(
    data: &[u8],
    export_offset: u64,
    export_count: usize,
    depends_offset: u64,
    import_class_name_idxs: &[usize],
    name_table: &[String],
) -> TextureMetadata {
    let mut meta = TextureMetadata::default();
    if export_count == 0 || depends_offset <= export_offset {
        return meta;
    }

    let bytes_per_entry = (depends_offset - export_offset) / export_count as u64;
    // SerialOffset is at entry_pos+36, occupies 8 bytes → need at least 44 bytes per entry.
    if bytes_per_entry < 44 {
        return meta;
    }

    for i in 0..export_count {
        let entry_pos = (export_offset + i as u64 * bytes_per_entry) as usize;
        if entry_pos + 44 > data.len() {
            break;
        }

        let class_name =
            export_class_name(data, entry_pos, import_class_name_idxs, name_table).unwrap_or("");
        if !TEXTURE_CLASSES.contains(&class_name) {
            continue;
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

        if serial_size <= 0 || serial_offset <= 0 {
            return meta;
        }

        let start = serial_offset as usize;
        let end = start.saturating_add(serial_size as usize);
        if end > data.len() {
            return meta;
        }

        let mut cur = Cursor::new(&data[start..end]);
        // Ignore the error: partial metadata captured before a malformed property is kept.
        let _ = scan_texture_props(&mut cur, name_table, &mut meta);
        return meta;
    }

    meta
}

// Walks the texture export's tagged-property stream until the "None" terminator, capturing the
// three enum properties.
//
// UE5 FPropertyTag layout (per property), confirmed against a UE5.4 Texture2D:
//   Name        FName  (i32 index + i32 number) = 8 bytes
//   [if Name == "None": stream ends]
//   Type        FName                            = 8 bytes
//   Size        i32  (serialized size of the value)
//   ArrayIndex  i32
//   <type-specific tag>:
//     StructProperty            -> StructName FName (8) + StructGuid FGuid (16)
//     BoolProperty              -> BoolVal u8 (1); the value lives here, Size == 0
//     Byte/EnumProperty         -> EnumName FName (8)
//     Array/SetProperty         -> InnerType FName (8)
//     MapProperty               -> KeyType FName (8) + ValueType FName (8)
//   HasPropertyGuid u8 (1)  [+ FGuid (16) if non-zero]
//   <value: Size bytes>     (absent for BoolProperty, whose value is in the tag)
fn scan_texture_props(
    cur: &mut Cursor<&[u8]>,
    name_table: &[String],
    out: &mut TextureMetadata,
) -> Result<(), ScanError> {
    loop {
        let name_idx = cur.read_i32::<LittleEndian>().map_err(map_io)? as usize;
        let _name_num = cur.read_i32::<LittleEndian>().map_err(map_io)?;

        let prop_name = name_table.get(name_idx).ok_or(ScanError::UnexpectedEof)?;
        if prop_name == "None" {
            return Ok(());
        }

        let type_idx = cur.read_i32::<LittleEndian>().map_err(map_io)? as usize;
        let _type_num = cur.read_i32::<LittleEndian>().map_err(map_io)?;
        let prop_type = name_table.get(type_idx).ok_or(ScanError::UnexpectedEof)?;

        let prop_size = cur.read_i32::<LittleEndian>().map_err(map_io)?;
        let _array_idx = cur.read_i32::<LittleEndian>().map_err(map_io)?;

        let is_enum = matches!(prop_type.as_str(), "ByteProperty" | "EnumProperty");

        // Skip the type-specific tag data that precedes the property-guid flag.
        match prop_type.as_str() {
            "StructProperty" => advance(cur, 24)?,
            "ByteProperty" | "EnumProperty" | "ArrayProperty" | "SetProperty" => advance(cur, 8)?,
            "MapProperty" => advance(cur, 16)?,
            "BoolProperty" => advance(cur, 1)?, // BoolVal; the value is held here
            _ => {}
        }

        // Optional per-property GUID (rare; flag is 0 in cooked content).
        let has_guid = cur.read_u8().map_err(map_io)?;
        if has_guid != 0 {
            advance(cur, 16)?;
        }

        if prop_type == "BoolProperty" {
            // Value already consumed as BoolVal in the tag (Size == 0).
            continue;
        }

        if is_enum && prop_size == 8 {
            // The enum value is serialized as an FName naming the entry (e.g. "TC_Default").
            let val_idx = cur.read_i32::<LittleEndian>().map_err(map_io)? as usize;
            let _val_num = cur.read_i32::<LittleEndian>().map_err(map_io)?;
            let value = name_table.get(val_idx).map(String::as_str);
            match prop_name.as_str() {
                "CompressionSettings" => out.compression = value.map(str::to_owned),
                "MipGenSettings" => out.mip_gen = value.map(str::to_owned),
                "LODGroup" => out.texture_group = value.map(str::to_owned),
                _ => {}
            }
        } else {
            advance(cur, prop_size.max(0) as u64)?;
        }
    }
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

    // name_table indices:
    //  0 = "None"
    //  1 = "Texture2D"
    //  2 = "CompressionSettings"
    //  3 = "ByteProperty"
    //  4 = "TextureCompressionSettings"   (enum type name in the tag)
    //  5 = "TC_Normalmap"                 (enum value)
    //  6 = "LODGroup"
    //  7 = "TextureGroup"                 (enum type name in the tag)
    //  8 = "TEXTUREGROUP_Character"       (enum value)
    //  9 = "MipGenSettings"
    // 10 = "TMGS_NoMipmaps"               (enum value)
    // 11 = "SRGB"
    // 12 = "BoolProperty"
    // 13 = "ImportedSize"
    // 14 = "StructProperty"
    // 15 = "IntPoint"
    fn name_table() -> Vec<String> {
        vec![
            "None".into(),
            "Texture2D".into(),
            "CompressionSettings".into(),
            "ByteProperty".into(),
            "TextureCompressionSettings".into(),
            "TC_Normalmap".into(),
            "LODGroup".into(),
            "TextureGroup".into(),
            "TEXTUREGROUP_Character".into(),
            "MipGenSettings".into(),
            "TMGS_NoMipmaps".into(),
            "SRGB".into(),
            "BoolProperty".into(),
            "ImportedSize".into(),
            "StructProperty".into(),
            "IntPoint".into(),
        ]
    }

    // ByteProperty enum: header + EnumName tag FName + property-guid flag (0) + value FName.
    fn byte_enum_prop(name_idx: i32, enum_type_idx: i32, value_idx: i32) -> Vec<u8> {
        let mut p = Vec::new();
        p.extend(fname(name_idx));
        p.extend(fname(3)); // "ByteProperty"
        p.extend(i32_le(8)); // Size = value FName = 8 bytes
        p.extend(i32_le(0)); // ArrayIndex
        p.extend(fname(enum_type_idx)); // EnumName tag
        p.push(0u8); // HasPropertyGuid = false
        p.extend(fname(value_idx)); // enum value FName
        p
    }

    // BoolProperty: header (Size 0) + BoolVal byte + property-guid flag.
    fn bool_prop(name_idx: i32, val: u8) -> Vec<u8> {
        let mut p = Vec::new();
        p.extend(fname(name_idx));
        p.extend(fname(12)); // "BoolProperty"
        p.extend(i32_le(0)); // Size = 0
        p.extend(i32_le(0)); // ArrayIndex
        p.push(val); // BoolVal
        p.push(0u8); // HasPropertyGuid = false
        p
    }

    // StructProperty header + StructName FName + FGuid + property-guid flag + value bytes.
    fn struct_prop(name_idx: i32, struct_name_idx: i32, value: &[u8]) -> Vec<u8> {
        let mut p = Vec::new();
        p.extend(fname(name_idx));
        p.extend(fname(14)); // "StructProperty"
        p.extend(i32_le(value.len() as i32)); // Size
        p.extend(i32_le(0)); // ArrayIndex
        p.extend(fname(struct_name_idx)); // StructName tag
        p.extend([0u8; 16]); // StructGuid
        p.push(0u8); // HasPropertyGuid = false
        p.extend_from_slice(value);
        p
    }

    fn none_terminator() -> Vec<u8> {
        fname(0)
    }

    // Wraps serial bytes in a fake data buffer with a single export table entry pointing to them.
    fn wrap_with_export_table(serial: Vec<u8>) -> Vec<u8> {
        let bytes_per_entry: usize = 112;
        let serial_offset = bytes_per_entry as i64;
        let serial_size = serial.len() as i64;

        let mut data: Vec<u8> = vec![0u8; bytes_per_entry];
        // ClassIndex = -1 → import[0]
        data[0..4].copy_from_slice(&(-1i32).to_le_bytes());
        data[28..36].copy_from_slice(&serial_size.to_le_bytes());
        data[36..44].copy_from_slice(&serial_offset.to_le_bytes());
        data.extend(serial);
        data
    }

    #[test]
    fn extract_texture_metadata_should_capture_compression_and_group_and_mip() {
        let mut serial = Vec::new();
        // A leading struct (FIntPoint ImportedSize) must be skipped without desyncing.
        serial.extend(struct_prop(13, 15, &[0u8; 8]));
        serial.extend(byte_enum_prop(2, 4, 5)); // CompressionSettings = TC_Normalmap
        serial.extend(byte_enum_prop(6, 7, 8)); // LODGroup = TEXTUREGROUP_Character
        serial.extend(byte_enum_prop(9, 4, 10)); // MipGenSettings = TMGS_NoMipmaps
        serial.extend(none_terminator());
        let data = wrap_with_export_table(serial);

        let meta = extract_texture_metadata(&data, 0, 1, 112, &[1usize], &name_table());

        assert_eq!(meta.compression.as_deref(), Some("TC_Normalmap"));
        assert_eq!(
            meta.texture_group.as_deref(),
            Some("TEXTUREGROUP_Character")
        );
        assert_eq!(meta.mip_gen.as_deref(), Some("TMGS_NoMipmaps"));
    }

    #[test]
    fn extract_texture_metadata_should_skip_unrelated_bool_property_before_enum() {
        let mut serial = Vec::new();
        serial.extend(bool_prop(11, 1)); // SRGB = true
        serial.extend(byte_enum_prop(2, 4, 5)); // CompressionSettings = TC_Normalmap
        serial.extend(none_terminator());
        let data = wrap_with_export_table(serial);

        let meta = extract_texture_metadata(&data, 0, 1, 112, &[1usize], &name_table());

        assert_eq!(meta.compression.as_deref(), Some("TC_Normalmap"));
        assert_eq!(meta.texture_group, None);
        assert_eq!(meta.mip_gen, None);
    }

    #[test]
    fn extract_texture_metadata_should_return_default_for_non_texture_export() {
        // import[0] → name_table[0] = "None" (not a texture class)
        let serial = none_terminator();
        let data = wrap_with_export_table(serial);

        let meta = extract_texture_metadata(&data, 0, 1, 112, &[0usize], &name_table());

        assert_eq!(meta, TextureMetadata::default());
    }

    #[test]
    fn extract_texture_metadata_should_keep_partial_on_truncated_stream() {
        // CompressionSettings captured, then the stream ends mid-property (no None terminator).
        let mut serial = Vec::new();
        serial.extend(byte_enum_prop(2, 4, 5)); // CompressionSettings = TC_Normalmap
        serial.extend(fname(6)); // dangling LODGroup name, then EOF
        let data = wrap_with_export_table(serial);

        let meta = extract_texture_metadata(&data, 0, 1, 112, &[1usize], &name_table());

        assert_eq!(meta.compression.as_deref(), Some("TC_Normalmap"));
        assert_eq!(meta.texture_group, None);
    }

    #[test]
    fn extract_texture_metadata_should_find_texture_in_a_later_export() {
        // Two export entries: entry 0 is non-texture, entry 1 is the Texture2D — exercises the
        // per-entry advance and the skip-then-match path.
        let mut serial = Vec::new();
        serial.extend(byte_enum_prop(2, 4, 5)); // CompressionSettings = TC_Normalmap
        serial.extend(none_terminator());

        let bytes_per_entry: usize = 112;
        let depends_offset = (2 * bytes_per_entry) as u64;
        let serial_offset = depends_offset as i64; // serial starts right after both entries
        let serial_size = serial.len() as i64;

        let mut data: Vec<u8> = vec![0u8; 2 * bytes_per_entry];
        // entry 0: ClassIndex -1 → import[0] = name_table[0] = "None" (non-texture)
        data[0..4].copy_from_slice(&(-1i32).to_le_bytes());
        // entry 1: ClassIndex -2 → import[1] = name_table[1] = "Texture2D"
        let e1 = bytes_per_entry;
        data[e1..e1 + 4].copy_from_slice(&(-2i32).to_le_bytes());
        data[e1 + 28..e1 + 36].copy_from_slice(&serial_size.to_le_bytes());
        data[e1 + 36..e1 + 44].copy_from_slice(&serial_offset.to_le_bytes());
        data.extend(serial);

        let meta = extract_texture_metadata(
            &data,
            0,
            2,
            depends_offset,
            &[0usize, 1usize],
            &name_table(),
        );

        assert_eq!(meta.compression.as_deref(), Some("TC_Normalmap"));
    }

    #[test]
    fn extract_texture_metadata_should_return_default_for_zero_export_count() {
        let meta = extract_texture_metadata(&[], 0, 0, 0, &[], &name_table());
        assert_eq!(meta, TextureMetadata::default());
    }

    #[test]
    fn is_texture_asset_should_match_texture_classes_only() {
        assert!(is_texture_asset(&AssetType::Texture2D));
        assert!(is_texture_asset(&AssetType::Unknown("TextureCube".into())));
        assert!(is_texture_asset(&AssetType::Unknown(
            "VolumeTexture".into()
        )));
        assert!(!is_texture_asset(&AssetType::Unknown("SoundWave".into())));
        assert!(!is_texture_asset(&AssetType::Material));
        assert!(!is_texture_asset(&AssetType::Blueprint));
    }
}
