use byteorder::{LittleEndian, ReadBytesExt};
use uasset_lens_shared::AssetPath;
use std::io::Cursor;

use super::{map_io, skip_fstring};
use crate::ScanError;

// FSoftObjectPath (UE5) layout per entry:
//   FTopLevelAssetPath:
//     PackageName FName (i32 index + i32 number) = 8 bytes
//     AssetName   FName (i32 index + i32 number) = 8 bytes
//   SubPathString FString (i32 length + bytes)   = variable
pub fn parse_soft_object_paths(
    data: &[u8],
    offset: u64,
    count: usize,
    name_table: &[String],
) -> Result<Vec<AssetPath>, ScanError> {
    if count == 0 {
        return Ok(Vec::new());
    }

    let mut cur = Cursor::new(data);
    cur.set_position(offset);

    let mut paths = Vec::with_capacity(count);

    for _ in 0..count {
        let pkg_idx = cur.read_i32::<LittleEndian>().map_err(map_io)?;
        let _pkg_num = cur.read_i32::<LittleEndian>().map_err(map_io)?;
        let _asset_idx = cur.read_i32::<LittleEndian>().map_err(map_io)?;
        let _asset_num = cur.read_i32::<LittleEndian>().map_err(map_io)?;
        skip_fstring(&mut cur)?;

        let pkg_name = name_table
            .get(pkg_idx as usize)
            .map(String::as_str)
            .unwrap_or("");

        // Filter before allocating: only /Game/ packages are tracked dependencies.
        if pkg_name.starts_with("/Game/")
            && let Ok(path) = AssetPath::new(pkg_name)
        {
            paths.push(path);
        }
    }

    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fstring_bytes(s: &str) -> Vec<u8> {
        // UTF-8 with null terminator; length includes the terminator
        let len = (s.len() + 1) as i32;
        let mut buf = len.to_le_bytes().to_vec();
        buf.extend_from_slice(s.as_bytes());
        buf.push(0);
        buf
    }

    fn make_fname(idx: i32) -> Vec<u8> {
        let mut buf = idx.to_le_bytes().to_vec();
        buf.extend_from_slice(&0i32.to_le_bytes()); // number
        buf
    }

    fn make_entry(pkg_idx: i32, sub_path: &str) -> Vec<u8> {
        let mut buf = make_fname(pkg_idx); // PackageName
        buf.extend(make_fname(0)); // AssetName
        buf.extend(make_fstring_bytes(sub_path)); // SubPathString
        buf
    }

    #[test]
    fn parse_soft_object_paths_should_return_empty_when_count_is_zero() {
        let result = parse_soft_object_paths(&[], 0, 0, &[]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn parse_soft_object_paths_should_return_game_paths_only() {
        let name_table: Vec<String> = vec![
            "/Script/Engine".to_string(),
            "/Game/Characters/BP_Hero".to_string(),
            "/Engine/BasicShapes/T_Default".to_string(),
        ];
        let mut data = Vec::new();
        data.extend(make_entry(0, "")); // /Script/ → excluded
        data.extend(make_entry(1, "")); // /Game/   → kept
        data.extend(make_entry(2, "")); // /Engine/ → excluded

        let result = parse_soft_object_paths(&data, 0, 3, &name_table).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_str(), "/Game/Characters/BP_Hero");
    }

    #[test]
    fn parse_soft_object_paths_should_skip_sub_path_fstring() {
        let name_table: Vec<String> = vec!["/Game/Levels/L_Main".to_string()];
        let data = make_entry(0, "SomeSubObject"); // non-empty SubPathString

        let result = parse_soft_object_paths(&data, 0, 1, &name_table).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].as_str(), "/Game/Levels/L_Main");
    }

    #[test]
    fn parse_soft_object_paths_should_return_all_entries_without_deduplication() {
        // Deduplication (e.g. for the same package referenced twice) is the caller's responsibility.
        let name_table: Vec<String> = vec!["/Game/Characters/BP_Hero".to_string()];
        let mut data = Vec::new();
        data.extend(make_entry(0, ""));
        data.extend(make_entry(0, "OtherSub"));

        let result = parse_soft_object_paths(&data, 0, 2, &name_table).unwrap();
        assert_eq!(result.len(), 2);
    }
}
