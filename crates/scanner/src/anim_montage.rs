use byteorder::{LittleEndian, ReadBytesExt};
use shared::{AssetPath, AssetType};
use std::io::Cursor;

use crate::ScanError;

// AnimMontage AnimSequence references are nested 1–4 levels deep:
//   Sections[i].AnimReference              (depth 1 struct element)
//   SlotAnimTracks[i].AnimTrack.AnimSegments[j].AnimReference (depth 3+)
// A flat scanner misses everything inside struct arrays, so we recurse.
const MAX_DEPTH: u8 = 6;

pub(crate) fn is_anim_montage_asset(asset_type: &AssetType) -> bool {
    matches!(asset_type, AssetType::AnimMontage)
}

// AnimSequence references inside AnimMontage are stored as soft refs, not in the
// import table, so the referenced assets appear dead without this extraction.
pub(crate) fn extract_anim_montage_soft_refs(
    data: &[u8],
    export_offset: u64,
    export_count: usize,
    depends_offset: u64,
    import_class_names: &[String],
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

    for i in 0..export_count {
        let entry_pos = (export_offset + i as u64 * bytes_per_entry) as usize;
        if entry_pos + 44 > data.len() {
            break;
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
        let class_name = import_class_names
            .get(imp_i)
            .map(String::as_str)
            .unwrap_or("");
        if class_name != "AnimMontage" {
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
            return Vec::new();
        }

        let start = serial_offset as usize;
        let end = start.saturating_add(serial_size as usize);
        if end > data.len() {
            return Vec::new();
        }

        return collect_soft_refs(&data[start..end], name_table);
    }

    Vec::new()
}

fn collect_soft_refs(serial_data: &[u8], name_table: &[String]) -> Vec<AssetPath> {
    let mut cur = Cursor::new(serial_data);
    let mut paths = Vec::new();
    let _ = scan_props_depth(&mut cur, name_table, &mut paths, 0);
    paths
}

// Reads one complete tagged-property stream (until "None") from `cur`,
// recursing into StructProperty and ArrayProperty-of-structs bodies.
fn scan_props_depth(
    cur: &mut Cursor<&[u8]>,
    name_table: &[String],
    out: &mut Vec<AssetPath>,
    depth: u8,
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

        let prop_size = cur.read_i64::<LittleEndian>().map_err(map_io)?;
        let _array_idx = cur.read_i32::<LittleEndian>().map_err(map_io)?;

        let value_start = cur.position();
        let value_end = value_start + prop_size.max(0) as u64;

        match prop_type.as_str() {
            "BoolProperty" => {
                // Tag: 1 byte holds the bool; prop_size == 0 (value section empty)
                cur.read_u8().map_err(map_io)?;
                advance(cur, prop_size.max(0) as u64)?;
            }
            "StructProperty" => {
                // Tag: struct name FName (8 bytes) + FGuid (16 bytes) = 24 bytes
                advance(cur, 24)?;
                if depth < MAX_DEPTH {
                    let body_end = cur.position() + prop_size.max(0) as u64;
                    let _ = scan_props_depth(cur, name_table, out, depth + 1);
                    // Advance to end regardless of whether recursion consumed all bytes.
                    if cur.position() < body_end {
                        cur.set_position(body_end);
                    }
                } else {
                    advance(cur, prop_size.max(0) as u64)?;
                }
            }
            "ArrayProperty" | "SetProperty" => {
                // Tag: inner element type FName (8 bytes)
                let inner_type_idx = cur.read_i32::<LittleEndian>().map_err(map_io)? as usize;
                let _inner_num = cur.read_i32::<LittleEndian>().map_err(map_io)?;
                let inner_type = name_table
                    .get(inner_type_idx)
                    .map(String::as_str)
                    .unwrap_or("");

                let array_end = cur.position() + prop_size.max(0) as u64;
                if depth < MAX_DEPTH
                    && (inner_type == "StructProperty"
                        || inner_type == "SoftObjectProperty"
                        || inner_type == "SoftClassProperty")
                {
                    let elem_count = cur.read_i32::<LittleEndian>().map_err(map_io)? as usize;

                    match inner_type {
                        "StructProperty" => {
                            // UE struct array: one-time header (struct_name FName 8B +
                            // FGuid 16B) follows elem_count, before all elements.
                            advance(cur, 24)?;
                            for _ in 0..elem_count {
                                if cur.position() >= array_end {
                                    break;
                                }
                                let _ = scan_props_depth(cur, name_table, out, depth + 1);
                            }
                        }
                        "SoftObjectProperty" | "SoftClassProperty" => {
                            for _ in 0..elem_count {
                                if cur.position() >= array_end {
                                    break;
                                }
                                read_soft_object_path(cur, name_table, out)?;
                            }
                        }
                        _ => {}
                    }
                }
                // Always advance to array_end to handle partial reads or skipped types.
                if cur.position() < array_end {
                    cur.set_position(array_end);
                }
            }
            "ByteProperty" | "EnumProperty" => {
                // Tag: enum name FName (8 bytes)
                advance(cur, 8)?;
                advance(cur, prop_size.max(0) as u64)?;
            }
            "MapProperty" => {
                // Tag: key type FName (8 bytes) + value type FName (8 bytes)
                advance(cur, 16)?;
                advance(cur, prop_size.max(0) as u64)?;
            }
            "SoftObjectProperty" | "SoftClassProperty" => {
                // No extra tag; value is FSoftObjectPath
                read_soft_object_path(cur, name_table, out)?;
            }
            "StrProperty" => {
                let len = cur.read_i32::<LittleEndian>().map_err(map_io)?;
                if len > 0 {
                    let str_start = cur.position() as usize;
                    let str_end = str_start + len as usize;
                    if str_end <= cur.get_ref().len() {
                        let s = std::str::from_utf8(
                            &cur.get_ref()[str_start..str_end.saturating_sub(1)],
                        )
                        .unwrap_or("");
                        if s.starts_with("/Game/")
                            && let Ok(p) = AssetPath::new(s)
                        {
                            out.push(p);
                        }
                    }
                    advance(cur, len as u64)?;
                } else if len < 0 {
                    advance(cur, (-len as u64) * 2)?;
                }
            }
            "NameProperty" => {
                let idx = cur.read_i32::<LittleEndian>().map_err(map_io)? as usize;
                let _num = cur.read_i32::<LittleEndian>().map_err(map_io)?;
                if let Some(n) = name_table.get(idx)
                    && n.starts_with("/Game/")
                    && let Ok(p) = AssetPath::new(n)
                {
                    out.push(p);
                }
            }
            _ => {
                advance(cur, prop_size.max(0) as u64)?;
            }
        }

        // Guard: if nothing consumed the value bytes, skip past them to avoid infinite loop.
        if cur.position() < value_end && prop_size > 0 {
            cur.set_position(value_end);
        }
    }
}

fn read_soft_object_path(
    cur: &mut Cursor<&[u8]>,
    name_table: &[String],
    out: &mut Vec<AssetPath>,
) -> Result<(), ScanError> {
    let pkg_idx = cur.read_i32::<LittleEndian>().map_err(map_io)? as usize;
    let _pkg_num = cur.read_i32::<LittleEndian>().map_err(map_io)?;
    let _asset_idx = cur.read_i32::<LittleEndian>().map_err(map_io)?;
    let _asset_num = cur.read_i32::<LittleEndian>().map_err(map_io)?;
    skip_fstring(cur)?;
    if let Some(pkg_name) = name_table.get(pkg_idx)
        && pkg_name.starts_with("/Game/")
        && let Ok(p) = AssetPath::new(pkg_name)
    {
        out.push(p);
    }
    Ok(())
}

fn advance(cur: &mut Cursor<&[u8]>, n: u64) -> Result<(), ScanError> {
    let new_pos = cur.position() + n;
    if new_pos > cur.get_ref().len() as u64 {
        return Err(ScanError::UnexpectedEof);
    }
    cur.set_position(new_pos);
    Ok(())
}

fn skip_fstring(cur: &mut Cursor<&[u8]>) -> Result<(), ScanError> {
    let len = cur.read_i32::<LittleEndian>().map_err(map_io)?;
    let byte_count: u64 = if len == 0 {
        0
    } else if len < 0 {
        (-len as u64) * 2
    } else {
        len as u64
    };
    advance(cur, byte_count)
}

fn map_io(e: std::io::Error) -> ScanError {
    if e.kind() == std::io::ErrorKind::UnexpectedEof {
        ScanError::UnexpectedEof
    } else {
        ScanError::Io(e)
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

    fn i64_le(v: i64) -> Vec<u8> {
        v.to_le_bytes().to_vec()
    }

    // name_table indices:
    //  0  = "None"
    //  1  = "AnimMontage"
    //  2  = "Sections"
    //  3  = "ArrayProperty"
    //  4  = "StructProperty"
    //  5  = "AnimReference"
    //  6  = "SoftObjectProperty"
    //  7  = "/Game/Anims/AS_Run"
    //  8  = "SlotAnimTracks"
    //  9  = "AnimTrack"
    // 10  = "AnimSegments"
    // 11  = "/Game/Anims/AS_Jump"
    // 12  = "FCompositeSection"
    // 13  = "FSlotAnimationTrack"
    // 14  = "FAnimTrack"
    // 15  = "FAnimSegment"
    // 16  = "SomeProp"
    // 17  = "IntProperty"
    fn name_table() -> Vec<String> {
        vec![
            "None".into(),
            "AnimMontage".into(),
            "Sections".into(),
            "ArrayProperty".into(),
            "StructProperty".into(),
            "AnimReference".into(),
            "SoftObjectProperty".into(),
            "/Game/Anims/AS_Run".into(),
            "SlotAnimTracks".into(),
            "AnimTrack".into(),
            "AnimSegments".into(),
            "/Game/Anims/AS_Jump".into(),
            "FCompositeSection".into(),
            "FSlotAnimationTrack".into(),
            "FAnimTrack".into(),
            "FAnimSegment".into(),
            "SomeProp".into(),
            "IntProperty".into(),
        ]
    }

    fn none_terminator() -> Vec<u8> {
        fname(0) // "None"
    }

    // Builds a tagged-property stream with a single SoftObjectProperty.
    fn soft_object_prop(name_idx: i32, pkg_idx: i32) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend(fname(name_idx)); // prop name
        v.extend(fname(6)); // type: "SoftObjectProperty"
        v.extend(i64_le(20)); // prop_size: FName(8)+FName(8)+FString(4)
        v.extend(i32_le(0)); // array_idx
        v.extend(fname(pkg_idx)); // PackageName FName
        v.extend(fname(0)); // AssetName FName
        v.extend(i32_le(0)); // SubPathString len = 0
        v
    }

    // Builds an ArrayProperty of structs where each element is a tagged-property
    // stream containing a single SoftObjectProperty, followed by a None terminator.
    // inner_struct_name_idx: name table index for the struct type name (used in the
    // inline struct header: struct_name FName + FGuid 16B).
    fn array_of_structs_with_soft_ref(
        array_name_idx: i32,
        inner_struct_name_idx: i32,
        pkg_idx: i32,
    ) -> Vec<u8> {
        // Build element body: a soft object prop + None terminator
        let mut elem = soft_object_prop(5, pkg_idx); // "AnimReference"
        elem.extend(none_terminator());

        // Inline struct array body: count(4) + struct_name FName(8) + FGuid(16) + elements
        let mut array_body = Vec::new();
        array_body.extend(i32_le(1)); // element count
        array_body.extend(fname(inner_struct_name_idx)); // struct name FName
        array_body.extend(vec![0u8; 16]); // FGuid (16 bytes)
        array_body.extend(elem);

        let prop_size = array_body.len() as i64;

        let mut v = Vec::new();
        v.extend(fname(array_name_idx)); // prop name
        v.extend(fname(3)); // type: "ArrayProperty"
        v.extend(i64_le(prop_size)); // prop_size
        v.extend(i32_le(0)); // array_idx
        v.extend(fname(4)); // inner type: "StructProperty" (tag, 8B)
        v.extend(array_body);
        v
    }

    fn wrap_with_export_table(serial: Vec<u8>) -> Vec<u8> {
        let bytes_per_entry: usize = 112;
        let serial_offset = bytes_per_entry as i64;
        let serial_size = serial.len() as i64;

        let mut data: Vec<u8> = vec![0u8; bytes_per_entry];
        data[0..4].copy_from_slice(&(-1i32).to_le_bytes()); // ClassIndex = -1 → import[0]
        data[28..36].copy_from_slice(&serial_size.to_le_bytes());
        data[36..44].copy_from_slice(&serial_offset.to_le_bytes());
        data.extend(serial);
        data
    }

    #[test]
    fn is_anim_montage_asset_should_return_true_for_anim_montage() {
        assert!(is_anim_montage_asset(&AssetType::AnimMontage));
    }

    #[test]
    fn is_anim_montage_asset_should_return_false_for_non_anim_montage() {
        assert!(!is_anim_montage_asset(&AssetType::Texture2D));
        assert!(!is_anim_montage_asset(&AssetType::Blueprint));
        assert!(!is_anim_montage_asset(&AssetType::AnimSequence));
    }

    #[test]
    fn extract_anim_montage_soft_refs_should_return_empty_for_zero_export_count() {
        let result = extract_anim_montage_soft_refs(&[], 0, 0, 0, &[], &name_table());
        assert!(result.is_empty());
    }

    #[test]
    fn extract_anim_montage_soft_refs_should_return_empty_for_non_anim_montage_export() {
        let serial = none_terminator();
        let data = wrap_with_export_table(serial);

        let result = extract_anim_montage_soft_refs(
            &data,
            0,
            1,
            112,
            &["Texture2D".to_string()],
            &name_table(),
        );

        assert!(result.is_empty());
    }

    #[test]
    fn extract_anim_montage_soft_refs_should_extract_top_level_soft_object_property() {
        // A SoftObjectProperty at depth 0 (directly in the AnimMontage export).
        let mut serial = soft_object_prop(5, 7); // AnimReference → /Game/Anims/AS_Run
        serial.extend(none_terminator());

        let data = wrap_with_export_table(serial);
        let result = extract_anim_montage_soft_refs(
            &data,
            0,
            1,
            112,
            &["AnimMontage".to_string()],
            &name_table(),
        );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_str(), "/Game/Anims/AS_Run");
    }

    #[test]
    fn extract_anim_montage_soft_refs_should_extract_soft_ref_inside_struct_array() {
        // Sections: ArrayProperty<StructProperty(FCompositeSection)>
        // Each element has AnimReference → /Game/Anims/AS_Run
        let mut serial = array_of_structs_with_soft_ref(2, 12, 7); // "Sections", FCompositeSection, AS_Run
        serial.extend(none_terminator());

        let data = wrap_with_export_table(serial);
        let result = extract_anim_montage_soft_refs(
            &data,
            0,
            1,
            112,
            &["AnimMontage".to_string()],
            &name_table(),
        );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_str(), "/Game/Anims/AS_Run");
    }

    #[test]
    fn extract_anim_montage_soft_refs_should_extract_refs_from_multiple_struct_elements() {
        // Build two elements with different AnimReference paths.
        let mut elem0 = soft_object_prop(5, 7); // AS_Run
        elem0.extend(none_terminator());
        let mut elem1 = soft_object_prop(5, 11); // AS_Jump
        elem1.extend(none_terminator());

        // Inline struct array: count(4) + struct_name FName(8) + FGuid(16) + elements
        let mut array_body = Vec::new();
        array_body.extend(i32_le(2));
        array_body.extend(fname(12)); // FCompositeSection
        array_body.extend(vec![0u8; 16]);
        array_body.extend(elem0);
        array_body.extend(elem1);

        let prop_size = array_body.len() as i64;
        let mut serial = Vec::new();
        serial.extend(fname(2)); // "Sections"
        serial.extend(fname(3)); // "ArrayProperty"
        serial.extend(i64_le(prop_size));
        serial.extend(i32_le(0));
        serial.extend(fname(4)); // inner: "StructProperty" (tag, 8B)
        serial.extend(array_body);
        serial.extend(none_terminator());

        let data = wrap_with_export_table(serial);
        let result = extract_anim_montage_soft_refs(
            &data,
            0,
            1,
            112,
            &["AnimMontage".to_string()],
            &name_table(),
        );

        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|p| p.as_str() == "/Game/Anims/AS_Run"));
        assert!(result.iter().any(|p| p.as_str() == "/Game/Anims/AS_Jump"));
    }

    #[test]
    fn extract_anim_montage_soft_refs_should_extract_deeply_nested_ref() {
        // SlotAnimTracks[i].AnimTrack.AnimSegments[j].AnimReference
        // depth 0: SlotAnimTracks ArrayProperty<StructProperty(FSlotAnimationTrack)>
        // depth 1: inside FSlotAnimationTrack → AnimTrack StructProperty(FAnimTrack)
        // depth 2: inside FAnimTrack → AnimSegments ArrayProperty<StructProperty(FAnimSegment)>
        // depth 3: inside FAnimSegment → AnimReference SoftObjectProperty

        // Depth 3 element: AnimReference soft ref + None
        let mut seg_elem = soft_object_prop(5, 11); // AS_Jump
        seg_elem.extend(none_terminator());

        // Depth 2 array: AnimSegments ArrayProperty<StructProperty(FAnimSegment)>
        let mut seg_array_body = Vec::new();
        seg_array_body.extend(i32_le(1));
        seg_array_body.extend(fname(15)); // FAnimSegment
        seg_array_body.extend(vec![0u8; 16]);
        seg_array_body.extend(seg_elem);

        let seg_prop_size = seg_array_body.len() as i64;
        let mut anim_segments_prop = Vec::new();
        anim_segments_prop.extend(fname(10)); // "AnimSegments"
        anim_segments_prop.extend(fname(3)); // "ArrayProperty"
        anim_segments_prop.extend(i64_le(seg_prop_size));
        anim_segments_prop.extend(i32_le(0));
        anim_segments_prop.extend(fname(4)); // inner: "StructProperty" (tag, 8B)
        anim_segments_prop.extend(seg_array_body);

        // Depth 1 struct: FAnimTrack tagged props (contains AnimSegments) + None
        let anim_track_body_size = (anim_segments_prop.len() + none_terminator().len()) as i64;
        let mut anim_track_struct = Vec::new();
        anim_track_struct.extend(fname(9)); // "AnimTrack"
        anim_track_struct.extend(fname(4)); // "StructProperty"
        anim_track_struct.extend(i64_le(anim_track_body_size));
        anim_track_struct.extend(i32_le(0));
        anim_track_struct.extend(fname(14)); // struct name: FAnimTrack
        anim_track_struct.extend(vec![0u8; 16]); // FGuid
        anim_track_struct.extend(anim_segments_prop);
        anim_track_struct.extend(none_terminator());

        // Depth 0 array element: FSlotAnimationTrack tagged props (contains AnimTrack) + None
        let mut slot_elem = Vec::new();
        slot_elem.extend(anim_track_struct);
        slot_elem.extend(none_terminator());

        // Depth 0 array: SlotAnimTracks ArrayProperty<StructProperty(FSlotAnimationTrack)>
        let mut slot_array_body = Vec::new();
        slot_array_body.extend(i32_le(1));
        slot_array_body.extend(fname(13)); // FSlotAnimationTrack
        slot_array_body.extend(vec![0u8; 16]);
        slot_array_body.extend(slot_elem);

        let slot_prop_size = slot_array_body.len() as i64;
        let mut serial = Vec::new();
        serial.extend(fname(8)); // "SlotAnimTracks"
        serial.extend(fname(3)); // "ArrayProperty"
        serial.extend(i64_le(slot_prop_size));
        serial.extend(i32_le(0));
        serial.extend(fname(4)); // inner: "StructProperty" (tag, 8B)
        serial.extend(slot_array_body);
        serial.extend(none_terminator());

        let data = wrap_with_export_table(serial);
        let result = extract_anim_montage_soft_refs(
            &data,
            0,
            1,
            112,
            &["AnimMontage".to_string()],
            &name_table(),
        );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_str(), "/Game/Anims/AS_Jump");
    }

    #[test]
    fn extract_anim_montage_soft_refs_should_skip_non_game_paths() {
        let mut serial = soft_object_prop(5, 1); // pkg = "AnimMontage" (not /Game/)
        serial.extend(none_terminator());

        let data = wrap_with_export_table(serial);
        let result = extract_anim_montage_soft_refs(
            &data,
            0,
            1,
            112,
            &["AnimMontage".to_string()],
            &name_table(),
        );

        assert!(result.is_empty());
    }
}
