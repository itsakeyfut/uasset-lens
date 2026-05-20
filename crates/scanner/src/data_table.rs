use byteorder::{LittleEndian, ReadBytesExt};
use shared::{AssetPath, AssetType};
use std::io::Cursor;

use crate::ScanError;

pub(crate) fn is_data_table_asset(asset_type: &AssetType) -> bool {
    matches!(asset_type, AssetType::DataTable)
}

// Walks every tagged-property stream in every DataTable row and collects
// FSoftObjectPath, FString, and FName values that start with "/Game/".
// These paths never appear in the import table, so assets referenced only
// through DataTable rows would otherwise be reported as dead.
pub(crate) fn extract_data_table_soft_refs(
    data: &[u8],
    export_offset: u64,
    export_count: usize,
    depends_offset: u64,
    import_class_name_idxs: &[usize],
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
        let class_name = import_class_name_idxs
            .get(imp_i)
            .and_then(|&idx| name_table.get(idx))
            .map(String::as_str)
            .unwrap_or("");
        if class_name != "DataTable" {
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

    // Skip UDataTable's own tagged properties (e.g. RowStruct ObjectProperty).
    if scan_props(&mut cur, name_table, &mut Vec::new()).is_err() {
        return Vec::new();
    }

    let row_count = match cur.read_i32::<LittleEndian>() {
        Ok(n) if n > 0 => n as usize,
        _ => return Vec::new(),
    };

    let mut paths = Vec::with_capacity(row_count);

    for _ in 0..row_count {
        // Row key: FName (index i32 + number i32)
        if cur.read_i32::<LittleEndian>().is_err() {
            break;
        }
        if cur.read_i32::<LittleEndian>().is_err() {
            break;
        }
        if scan_props(&mut cur, name_table, &mut paths).is_err() {
            break;
        }
    }

    paths
}

// Reads one complete tagged-property stream (until "None") from `cur`.
// Soft asset references found are appended to `out`.
fn scan_props(
    cur: &mut Cursor<&[u8]>,
    name_table: &[String],
    out: &mut Vec<AssetPath>,
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

        match prop_type.as_str() {
            "BoolProperty" => {
                // Tag: 1 byte holds the bool; prop_size == 0 (value section empty)
                cur.read_u8().map_err(map_io)?;
                advance(cur, prop_size.max(0) as u64)?;
            }
            "StructProperty" => {
                // Tag: struct name FName (8 bytes) + FGuid (16 bytes) = 24 bytes
                advance(cur, 24)?;
                advance(cur, prop_size.max(0) as u64)?;
            }
            "ArrayProperty" | "SetProperty" => {
                // Tag: inner element type FName (8 bytes)
                advance(cur, 8)?;
                advance(cur, prop_size.max(0) as u64)?;
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
                // No extra tag; value is FSoftObjectPath:
                //   FTopLevelAssetPath = { PackageName FName (8), AssetName FName (8) }
                //   SubPathString FString (variable)
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
            }
            "StrProperty" => {
                // No extra tag; value is FString. Check if it encodes a /Game/ path.
                let len = cur.read_i32::<LittleEndian>().map_err(map_io)?;
                if len > 0 {
                    let str_start = cur.position() as usize;
                    let str_end = str_start + len as usize;
                    if str_end <= cur.get_ref().len() {
                        // Null-terminated UTF-8; trim the trailing null byte.
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
                // No extra tag; value is FName (8 bytes). Collect if it is a /Game/ path.
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
    }
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
    //  0 = "None"
    //  1 = "DataTable"
    //  2 = "AssetRef"
    //  3 = "SoftObjectProperty"
    //  4 = "/Game/Icons/T_Icon"
    //  5 = "RowKey"
    //  6 = "StrProperty"
    //  7 = "PathStr"
    //  8 = "NameProperty"
    //  9 = "NameRef"
    // 10 = "/Game/Meshes/SM_Cube"
    // 11 = "ObjectProperty"
    // 12 = "RowStruct"
    // 13 = "Texture2D"
    fn name_table() -> Vec<String> {
        vec![
            "None".into(),
            "DataTable".into(),
            "AssetRef".into(),
            "SoftObjectProperty".into(),
            "/Game/Icons/T_Icon".into(),
            "RowKey".into(),
            "StrProperty".into(),
            "PathStr".into(),
            "NameProperty".into(),
            "NameRef".into(),
            "/Game/Meshes/SM_Cube".into(),
            "ObjectProperty".into(),
            "RowStruct".into(),
            "Texture2D".into(),
        ]
    }

    // Builds a minimal DataTable serial blob:
    //   own_props    – raw bytes for UDataTable's own tagged props (before row_count)
    //   rows         – list of (row_key_name_idx, raw row property bytes)
    fn build_serial(own_props: Vec<u8>, rows: Vec<(i32, Vec<u8>)>) -> Vec<u8> {
        let mut serial = own_props;
        serial.extend(i32_le(rows.len() as i32));
        for (key_idx, props) in rows {
            serial.extend(fname(key_idx));
            serial.extend(props);
        }
        serial
    }

    // Wraps serial bytes in a fake data buffer with a single export table entry
    // pointing to the serial data.
    fn wrap_with_export_table(serial: Vec<u8>, _import_class_name: &str) -> Vec<u8> {
        // bytes_per_entry must be large enough to hold SerialOffset at offset 44.
        let bytes_per_entry: usize = 112;
        let serial_offset = bytes_per_entry as i64;
        let serial_size = serial.len() as i64;

        let mut data: Vec<u8> = vec![0u8; bytes_per_entry];
        // ClassIndex = -1 → import[0] = import_class_name
        data[0..4].copy_from_slice(&(-1i32).to_le_bytes());
        data[28..36].copy_from_slice(&serial_size.to_le_bytes());
        data[36..44].copy_from_slice(&serial_offset.to_le_bytes());
        data.extend(serial);
        data
    }

    fn none_terminator() -> Vec<u8> {
        fname(0) // "None"
    }

    #[test]
    fn extract_data_table_soft_refs_should_extract_soft_object_property_path() {
        let mut row_props = Vec::new();
        // SoftObjectProperty: AssetRef = /Game/Icons/T_Icon
        row_props.extend(fname(2)); // name: "AssetRef"
        row_props.extend(fname(3)); // type: "SoftObjectProperty"
        row_props.extend(i64_le(20)); // size = FName(8)+FName(8)+FString(4) = 20
        row_props.extend(i32_le(0)); // array_idx
        row_props.extend(fname(4)); // PackageName: "/Game/Icons/T_Icon"
        row_props.extend(fname(0)); // AssetName: "None"
        row_props.extend(i32_le(0)); // SubPathString len = 0
        row_props.extend(none_terminator());

        let own_props = none_terminator();
        let serial = build_serial(own_props, vec![(5, row_props)]);
        let data = wrap_with_export_table(serial, "DataTable");

        let result = extract_data_table_soft_refs(
            &data,
            0,
            1,
            112,
            &[1usize], // import[0] → name_table[1] = "DataTable"
            &name_table(),
        );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_str(), "/Game/Icons/T_Icon");
    }

    #[test]
    fn extract_data_table_soft_refs_should_extract_str_property_path() {
        let path_str = "/Game/Meshes/SM_Cube\0";
        let path_bytes = path_str.as_bytes();

        let mut row_props = Vec::new();
        // StrProperty: PathStr = "/Game/Meshes/SM_Cube"
        row_props.extend(fname(7)); // name: "PathStr"
        row_props.extend(fname(6)); // type: "StrProperty"
        let str_total: i64 = 4 + path_bytes.len() as i64;
        row_props.extend(i64_le(str_total));
        row_props.extend(i32_le(0)); // array_idx
        row_props.extend(i32_le(path_bytes.len() as i32)); // FString len
        row_props.extend_from_slice(path_bytes);
        row_props.extend(none_terminator());

        let own_props = none_terminator();
        let serial = build_serial(own_props, vec![(5, row_props)]);
        let data = wrap_with_export_table(serial, "DataTable");

        let result = extract_data_table_soft_refs(
            &data,
            0,
            1,
            112,
            &[1usize], // import[0] → name_table[1] = "DataTable"
            &name_table(),
        );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_str(), "/Game/Meshes/SM_Cube");
    }

    #[test]
    fn extract_data_table_soft_refs_should_extract_name_property_game_path() {
        let mut row_props = Vec::new();
        // NameProperty: NameRef = /Game/Meshes/SM_Cube (idx 10)
        row_props.extend(fname(9)); // name: "NameRef"
        row_props.extend(fname(8)); // type: "NameProperty"
        row_props.extend(i64_le(8)); // size = FName(8)
        row_props.extend(i32_le(0)); // array_idx
        row_props.extend(fname(10)); // value: "/Game/Meshes/SM_Cube"
        row_props.extend(none_terminator());

        let own_props = none_terminator();
        let serial = build_serial(own_props, vec![(5, row_props)]);
        let data = wrap_with_export_table(serial, "DataTable");

        let result = extract_data_table_soft_refs(
            &data,
            0,
            1,
            112,
            &[1usize], // import[0] → name_table[1] = "DataTable"
            &name_table(),
        );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_str(), "/Game/Meshes/SM_Cube");
    }

    #[test]
    fn extract_data_table_soft_refs_should_skip_non_game_paths() {
        let mut row_props = Vec::new();
        // SoftObjectProperty with a /Engine/ path — should be filtered out
        row_props.extend(fname(2)); // name: "AssetRef"
        row_props.extend(fname(3)); // type: "SoftObjectProperty"
        row_props.extend(i64_le(20));
        row_props.extend(i32_le(0)); // array_idx
        // PackageName idx=1 "DataTable" — does not start with /Game/
        row_props.extend(fname(1));
        row_props.extend(fname(0)); // AssetName: "None"
        row_props.extend(i32_le(0)); // SubPathString len = 0
        row_props.extend(none_terminator());

        let own_props = none_terminator();
        let serial = build_serial(own_props, vec![(5, row_props)]);
        let data = wrap_with_export_table(serial, "DataTable");

        let result = extract_data_table_soft_refs(
            &data,
            0,
            1,
            112,
            &[1usize], // import[0] → name_table[1] = "DataTable"
            &name_table(),
        );

        assert!(result.is_empty());
    }

    #[test]
    fn extract_data_table_soft_refs_should_handle_multiple_rows() {
        let mut row0 = Vec::new();
        row0.extend(fname(2)); // name: "AssetRef"
        row0.extend(fname(3)); // type: "SoftObjectProperty"
        row0.extend(i64_le(20));
        row0.extend(i32_le(0));
        row0.extend(fname(4)); // PackageName: "/Game/Icons/T_Icon"
        row0.extend(fname(0));
        row0.extend(i32_le(0));
        row0.extend(none_terminator());

        let mut row1 = Vec::new();
        row1.extend(fname(2)); // name: "AssetRef"
        row1.extend(fname(3)); // type: "SoftObjectProperty"
        row1.extend(i64_le(20));
        row1.extend(i32_le(0));
        row1.extend(fname(10)); // PackageName: "/Game/Meshes/SM_Cube"
        row1.extend(fname(0));
        row1.extend(i32_le(0));
        row1.extend(none_terminator());

        let own_props = none_terminator();
        let serial = build_serial(own_props, vec![(5, row0), (5, row1)]);
        let data = wrap_with_export_table(serial, "DataTable");

        let result = extract_data_table_soft_refs(
            &data,
            0,
            1,
            112,
            &[1usize], // import[0] → name_table[1] = "DataTable"
            &name_table(),
        );

        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|p| p.as_str() == "/Game/Icons/T_Icon"));
        assert!(result.iter().any(|p| p.as_str() == "/Game/Meshes/SM_Cube"));
    }

    #[test]
    fn extract_data_table_soft_refs_should_skip_own_props_before_rows() {
        // UDataTable has a RowStruct ObjectProperty in its own props.
        let mut own_props = Vec::new();
        // ObjectProperty: RowStruct → package index -1 (size 4)
        own_props.extend(fname(12)); // name: "RowStruct"
        own_props.extend(fname(11)); // type: "ObjectProperty"
        own_props.extend(i64_le(4)); // size = 4
        own_props.extend(i32_le(0)); // array_idx
        own_props.extend(i32_le(-1)); // FPackageIndex
        own_props.extend(none_terminator());

        let mut row_props = Vec::new();
        row_props.extend(fname(2)); // name: "AssetRef"
        row_props.extend(fname(3)); // type: "SoftObjectProperty"
        row_props.extend(i64_le(20));
        row_props.extend(i32_le(0));
        row_props.extend(fname(4)); // PackageName: "/Game/Icons/T_Icon"
        row_props.extend(fname(0));
        row_props.extend(i32_le(0));
        row_props.extend(none_terminator());

        let serial = build_serial(own_props, vec![(5, row_props)]);
        let data = wrap_with_export_table(serial, "DataTable");

        let result = extract_data_table_soft_refs(
            &data,
            0,
            1,
            112,
            &[1usize], // import[0] → name_table[1] = "DataTable"
            &name_table(),
        );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_str(), "/Game/Icons/T_Icon");
    }

    #[test]
    fn extract_data_table_soft_refs_should_return_empty_for_non_data_table_export() {
        let serial = none_terminator(); // minimal serial
        let data = wrap_with_export_table(serial, "Texture2D");

        let result = extract_data_table_soft_refs(
            &data,
            0,
            1,
            112,
            &[13usize], // import[0] → name_table[13] = "Texture2D"
            &name_table(),
        );

        assert!(result.is_empty());
    }

    #[test]
    fn extract_data_table_soft_refs_should_return_empty_for_zero_export_count() {
        let result = extract_data_table_soft_refs(&[], 0, 0, 0, &[], &name_table());
        assert!(result.is_empty());
    }

    #[test]
    fn is_data_table_asset_should_return_true_for_data_table() {
        assert!(is_data_table_asset(&AssetType::DataTable));
    }

    #[test]
    fn is_data_table_asset_should_return_false_for_non_data_table() {
        assert!(!is_data_table_asset(&AssetType::Texture2D));
        assert!(!is_data_table_asset(&AssetType::Blueprint));
        assert!(!is_data_table_asset(&AssetType::StaticMesh));
    }
}
