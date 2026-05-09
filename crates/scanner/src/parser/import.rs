use byteorder::{LittleEndian, ReadBytesExt};
use shared::AssetPath;
use std::io::Cursor;

use crate::ScanError;

// FObjectImport layout: ClassPkg(FName=8) + ClassName(FName=8) + OuterIdx(4) + ObjName(FName=8) + PkgName(FName=8) + bOptional(4) = 40 bytes
pub fn parse_import_table(
    data: &[u8],
    offset: u64,
    count: usize,
    name_table: &[String],
) -> Result<Vec<AssetPath>, ScanError> {
    let mut cur = Cursor::new(data);
    cur.set_position(offset);

    let mut deps = Vec::new();

    for _ in 0..count {
        let _cp_idx = cur.read_i32::<LittleEndian>().map_err(map_io)?;
        let _cp_num = cur.read_i32::<LittleEndian>().map_err(map_io)?;
        let _cn_idx = cur.read_i32::<LittleEndian>().map_err(map_io)?;
        let _cn_num = cur.read_i32::<LittleEndian>().map_err(map_io)?;
        let outer_index = cur.read_i32::<LittleEndian>().map_err(map_io)?;
        let on_idx = cur.read_i32::<LittleEndian>().map_err(map_io)?;
        let _on_num = cur.read_i32::<LittleEndian>().map_err(map_io)?;
        let _pn_idx = cur.read_i32::<LittleEndian>().map_err(map_io)?;
        let _pn_num = cur.read_i32::<LittleEndian>().map_err(map_io)?;
        let _import_optional = cur.read_i32::<LittleEndian>().map_err(map_io)?;

        // Only package-level entries (OuterIndex == 0) carry the full package path
        if outer_index != 0 {
            continue;
        }

        let name = name_table
            .get(on_idx as usize)
            .map(String::as_str)
            .unwrap_or("");

        // Filter before allocating: discard /Script/ and /Engine/, keep only /Game/
        if !name.starts_with("/Game/") {
            continue;
        }

        if let Ok(path) = AssetPath::new(name) {
            deps.push(path);
        }
    }

    Ok(deps)
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
    use crate::parser::name_table::parse_name_table;
    use std::fs;

    const FIXTURES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures");

    fn read_fixture(name: &str) -> Vec<u8> {
        fs::read(format!("{FIXTURES_DIR}/{name}")).unwrap()
    }

    // Builds a single 40-byte FObjectImport entry with the given ObjectName index and
    // OuterIndex; all other fields are zero.
    fn make_import_entry(object_name_idx: i32, outer_index: i32) -> Vec<u8> {
        let mut buf = Vec::with_capacity(40);
        buf.extend_from_slice(&0i32.to_le_bytes()); // ClassPackage idx
        buf.extend_from_slice(&0i32.to_le_bytes()); // ClassPackage num
        buf.extend_from_slice(&0i32.to_le_bytes()); // ClassName idx
        buf.extend_from_slice(&0i32.to_le_bytes()); // ClassName num
        buf.extend_from_slice(&outer_index.to_le_bytes()); // OuterIndex
        buf.extend_from_slice(&object_name_idx.to_le_bytes()); // ObjectName idx
        buf.extend_from_slice(&0i32.to_le_bytes()); // ObjectName num
        buf.extend_from_slice(&0i32.to_le_bytes()); // PackageName idx
        buf.extend_from_slice(&0i32.to_le_bytes()); // PackageName num
        buf.extend_from_slice(&0i32.to_le_bytes()); // bImportOptional
        buf
    }

    #[test]
    fn parse_import_table_should_return_game_paths_for_bp_simple_fixture() {
        let data = read_fixture("valid/BP_Simple.uasset");
        let name_table = parse_name_table(&data, 537, 113).unwrap();
        let deps = parse_import_table(&data, 3253, 22, &name_table).unwrap();
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].as_str(), "/Game/PlayerCharacter/BP_HeroCharacter");
        assert_eq!(deps[1].as_str(), "/Game/PlayerCharacter/BP_HeroController");
    }

    #[test]
    fn parse_import_table_should_exclude_script_and_engine_paths() {
        // name_table: 3 entries — /Script/, /Engine/, /Game/
        let name_table: Vec<String> = vec![
            "/Script/Engine".to_string(),
            "/Engine/Content/BasicShapes/T_DefaultNormal".to_string(),
            "/Game/Characters/BP_Hero".to_string(),
        ];
        let mut import_data = Vec::new();
        import_data.extend(make_import_entry(0, 0)); // /Script/Engine → excluded
        import_data.extend(make_import_entry(1, 0)); // /Engine/... → excluded
        import_data.extend(make_import_entry(2, 0)); // /Game/... → kept

        let deps = parse_import_table(&import_data, 0, 3, &name_table).unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].as_str(), "/Game/Characters/BP_Hero");
    }

    #[test]
    fn parse_import_table_should_ignore_non_package_level_entries() {
        // Entries with OuterIndex != 0 (sub-objects) must not appear in the result
        let name_table: Vec<String> = vec![
            "/Game/Characters/BP_Hero".to_string(),
            "BP_Hero_C".to_string(), // sub-object name
        ];
        let mut import_data = Vec::new();
        import_data.extend(make_import_entry(0, 0)); // package-level → kept
        import_data.extend(make_import_entry(1, -1)); // sub-object → excluded

        let deps = parse_import_table(&import_data, 0, 2, &name_table).unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].as_str(), "/Game/Characters/BP_Hero");
    }
}
