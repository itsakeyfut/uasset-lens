// Tagged-property stream scanner for extracting FSoftObjectPath values.
//
// UE5 tagged property stream layout (per property):
//   PropName  FName  (i32 index + i32 number) = 8 bytes
//   PropType  FName  (i32 index + i32 number) = 8 bytes
//   PropSize  i64                             = 8 bytes
//   ArrayIdx  i32                             = 4 bytes
//   [type-specific tag bytes]
//   [value bytes]
// Stream ends when PropName resolves to "None".
//
// Used by both anim_montage and level_sequence to avoid duplicating ~170 lines.

use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;
use uasset_lens_shared::AssetPath;

use super::{advance, map_io, skip_fstring};
use crate::ScanError;

const MAX_DEPTH: u8 = 6;

pub(crate) fn scan_props_depth(
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
                    // Widen before negating: -len overflows i32 when len == i32::MIN.
                    advance(cur, (-(i64::from(len))) as u64 * 2)?;
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
