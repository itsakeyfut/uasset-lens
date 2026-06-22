pub mod export;
pub mod header;
pub mod import;
pub mod name_table;
pub mod properties;
pub mod soft_object_paths;
pub(crate) mod tagged_props;

#[cfg(test)]
pub mod test_utils;

use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

use crate::ScanError;

pub(crate) fn map_io(e: std::io::Error) -> ScanError {
    if e.kind() == std::io::ErrorKind::UnexpectedEof {
        ScanError::UnexpectedEof
    } else {
        ScanError::Io(e)
    }
}

pub(crate) fn advance(cur: &mut Cursor<&[u8]>, n: u64) -> Result<(), ScanError> {
    let new_pos = cur.position() + n;
    if new_pos > cur.get_ref().len() as u64 {
        return Err(ScanError::UnexpectedEof);
    }
    cur.set_position(new_pos);
    Ok(())
}

// Keeps FString skip logic in one place so header and soft_object_paths stay in sync.
pub(crate) fn skip_fstring(cur: &mut Cursor<&[u8]>) -> Result<(), ScanError> {
    let len = cur.read_i32::<LittleEndian>().map_err(map_io)?;
    let byte_count: u64 = if len == 0 {
        0
    } else if len < 0 {
        // UTF-16LE: negative length encodes char count; 2 bytes per char
        (-len as u64) * 2
    } else {
        // UTF-8/ASCII including null terminator
        len as u64
    };
    advance(cur, byte_count)
}

/// Reads the export entry's `ClassIndex` (i32 at `entry_pos`) and resolves it to the import's
/// class name. Returns `None` when `entry_pos + 4` is past the end of `data`, for an export-local
/// class (`ClassIndex >= 0`), or for an out-of-range import index. All export-table scanners
/// (asset-type, blueprint, data-table, anim-montage, material) share this resolution but keep
/// their own per-entry bounds checks and loop bodies.
pub(crate) fn export_class_name<'a>(
    data: &[u8],
    entry_pos: usize,
    import_class_name_idxs: &[usize],
    name_table: &'a [String],
) -> Option<&'a str> {
    if entry_pos + 4 > data.len() {
        return None;
    }
    let class_index = i32::from_le_bytes([
        data[entry_pos],
        data[entry_pos + 1],
        data[entry_pos + 2],
        data[entry_pos + 3],
    ]);
    if class_index >= 0 {
        return None;
    }
    // Widen before negating: -class_index overflows i32 when class_index == i32::MIN.
    let imp_i = (-(i64::from(class_index)) - 1) as usize;
    import_class_name_idxs
        .get(imp_i)
        .and_then(|&idx| name_table.get(idx))
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::export_class_name;

    #[test]
    fn export_class_name_should_resolve_import_class() {
        // ClassIndex = -1 → import 0 → name_table[idxs[0] = 2] = "DataTable"
        let data = (-1i32).to_le_bytes().to_vec();
        let idxs = [2usize];
        let names = ["A".to_owned(), "B".to_owned(), "DataTable".to_owned()];
        assert_eq!(
            export_class_name(&data, 0, &idxs, &names),
            Some("DataTable")
        );
    }

    #[test]
    fn export_class_name_should_return_none_for_export_local_class() {
        // ClassIndex >= 0 is an export-local class, not an import → None
        let data = 5i32.to_le_bytes().to_vec();
        let idxs = [0usize];
        let names = ["X".to_owned()];
        assert_eq!(export_class_name(&data, 0, &idxs, &names), None);
    }

    #[test]
    fn export_class_name_should_return_none_for_out_of_range_import_index() {
        // ClassIndex = -10 → import index 9, but only one import → None
        let data = (-10i32).to_le_bytes().to_vec();
        let idxs = [0usize];
        let names = ["X".to_owned()];
        assert_eq!(export_class_name(&data, 0, &idxs, &names), None);
    }

    #[test]
    fn export_class_name_should_return_none_when_name_table_index_out_of_range() {
        // ClassIndex = -1 → import 0 → idxs[0] = 5, but name_table has one entry → None
        let data = (-1i32).to_le_bytes().to_vec();
        let idxs = [5usize];
        let names = ["X".to_owned()];
        assert_eq!(export_class_name(&data, 0, &idxs, &names), None);
    }

    #[test]
    fn export_class_name_should_return_none_when_entry_pos_out_of_bounds() {
        // Only 3 bytes available but the ClassIndex read needs 4 → None (no panic)
        let data = [0u8, 0, 0];
        assert_eq!(export_class_name(&data, 0, &[], &[]), None);
    }

    #[test]
    fn export_class_name_should_return_none_for_i32_min_class_index() {
        // ClassIndex = i32::MIN must not overflow on negation in debug builds → None (no panic)
        let data = i32::MIN.to_le_bytes().to_vec();
        assert_eq!(export_class_name(&data, 0, &[], &[]), None);
    }
}
