use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

use super::{advance, map_io};
use crate::ScanError;

// UE5 tagged property stream layout (per-property):
//   PropertyName  FName (i32 index + i32 number) = 8 bytes
//   if name == "None": end of stream
//   PropertyType  FName (i32 index + i32 number) = 8 bytes
//   PropertySize  i64                            = 8 bytes  (byte count of value section)
//   ArrayIndex    i32                            = 4 bytes
//   Tag           bytes  (type-specific; see match arms below)
//   Value         PropertySize bytes
//
// Unknown property types are skipped by advancing exactly PropertySize bytes.
// class_name in Object stores the raw FPackageIndex (i32) as a string; resolving
// it to an import/export name requires the import table and is deferred to Issue #2.

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedProperty {
    Int { name: String, value: i32 },
    Bool { name: String, value: bool },
    Array { name: String, count: usize },
    Object { name: String, class_name: String },
    Skipped { name: String },
}

pub fn parse_properties(
    data: &[u8],
    offset: u64,
    name_table: &[String],
) -> Result<Vec<ParsedProperty>, ScanError> {
    let mut cur = Cursor::new(data);
    cur.set_position(offset);
    let mut result = Vec::new();

    loop {
        let name_idx = usize::try_from(cur.read_i32::<LittleEndian>().map_err(map_io)?)
            .map_err(|_| ScanError::UnexpectedEof)?;
        let _name_num = cur.read_i32::<LittleEndian>().map_err(map_io)?;
        let prop_name = name_table
            .get(name_idx)
            .cloned()
            .ok_or(ScanError::UnexpectedEof)?;

        if prop_name == "None" {
            break;
        }

        let type_idx = usize::try_from(cur.read_i32::<LittleEndian>().map_err(map_io)?)
            .map_err(|_| ScanError::UnexpectedEof)?;
        let _type_num = cur.read_i32::<LittleEndian>().map_err(map_io)?;
        let prop_type = name_table
            .get(type_idx)
            .cloned()
            .ok_or(ScanError::UnexpectedEof)?;

        let prop_size = cur.read_i64::<LittleEndian>().map_err(map_io)?;
        let _array_idx = cur.read_i32::<LittleEndian>().map_err(map_io)?;

        let parsed = match prop_type.as_str() {
            "BoolProperty" => {
                // Tag: 1 byte holds the bool; PropertySize = 0 (value section is empty)
                let byte = cur.read_u8().map_err(map_io)?;
                advance(&mut cur, prop_size as u64)?;
                ParsedProperty::Bool {
                    name: prop_name,
                    value: byte != 0,
                }
            }
            "ArrayProperty" => {
                // Tag: inner element type FName (8 bytes)
                let _inner_idx = cur.read_i32::<LittleEndian>().map_err(map_io)?;
                let _inner_num = cur.read_i32::<LittleEndian>().map_err(map_io)?;
                // Value: i32 element count followed by element data (we skip the rest)
                let count = cur.read_i32::<LittleEndian>().map_err(map_io)? as usize;
                let remaining = (prop_size - 4).max(0) as u64;
                advance(&mut cur, remaining)?;
                ParsedProperty::Array {
                    name: prop_name,
                    count,
                }
            }
            "IntProperty" => {
                // No tag; value is i32 (PropertySize = 4)
                let value = cur.read_i32::<LittleEndian>().map_err(map_io)?;
                let remaining = (prop_size - 4).max(0) as u64;
                advance(&mut cur, remaining)?;
                ParsedProperty::Int {
                    name: prop_name,
                    value,
                }
            }
            "ObjectProperty" => {
                // No tag; value is i32 FPackageIndex (PropertySize = 4)
                let pkg_idx = cur.read_i32::<LittleEndian>().map_err(map_io)?;
                let remaining = (prop_size - 4).max(0) as u64;
                advance(&mut cur, remaining)?;
                ParsedProperty::Object {
                    name: prop_name,
                    class_name: pkg_idx.to_string(),
                }
            }
            _ => {
                advance(&mut cur, prop_size.max(0) as u64)?;
                ParsedProperty::Skipped { name: prop_name }
            }
        };

        result.push(parsed);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Encodes an FName as 8 bytes: i32 name-table index (LE) + i32 number (LE, always 0).
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
    //   0 = "None"
    //   1 = "NodeCount"
    //   2 = "IsActive"
    //   3 = "Components"
    //   4 = "IntProperty"
    //   5 = "BoolProperty"
    //   6 = "ArrayProperty"
    //   7 = "Inner"
    //   8 = "FloatVal"
    //   9 = "FloatProperty"
    //  10 = "TargetObject"
    //  11 = "ObjectProperty"
    fn name_table() -> Vec<String> {
        vec![
            "None".into(),
            "NodeCount".into(),
            "IsActive".into(),
            "Components".into(),
            "IntProperty".into(),
            "BoolProperty".into(),
            "ArrayProperty".into(),
            "Inner".into(),
            "FloatVal".into(),
            "FloatProperty".into(),
            "TargetObject".into(),
            "ObjectProperty".into(),
        ]
    }

    #[test]
    fn parse_properties_should_parse_int_bool_array_followed_by_none() {
        let mut data = Vec::new();

        // IntProperty: NodeCount = 42
        data.extend(fname(1)); // name: "NodeCount"
        data.extend(fname(4)); // type: "IntProperty"
        data.extend(i64_le(4)); // size = 4
        data.extend(i32_le(0)); // array_index
        data.extend(i32_le(42)); // value

        // BoolProperty: IsActive = true
        data.extend(fname(2)); // name: "IsActive"
        data.extend(fname(5)); // type: "BoolProperty"
        data.extend(i64_le(0)); // size = 0
        data.extend(i32_le(0)); // array_index
        data.push(1u8); // tag: bool = true

        // ArrayProperty: Components = 7 elements
        data.extend(fname(3)); // name: "Components"
        data.extend(fname(6)); // type: "ArrayProperty"
        data.extend(i64_le(4)); // size = 4 (just the count i32)
        data.extend(i32_le(0)); // array_index
        data.extend(fname(7)); // tag: inner type "Inner"
        data.extend(i32_le(7)); // value: count

        // Terminator
        data.extend(fname(0)); // name: "None"

        let result = parse_properties(&data, 0, &name_table()).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(
            result[0],
            ParsedProperty::Int {
                name: "NodeCount".into(),
                value: 42
            }
        );
        assert_eq!(
            result[1],
            ParsedProperty::Bool {
                name: "IsActive".into(),
                value: true
            }
        );
        assert_eq!(
            result[2],
            ParsedProperty::Array {
                name: "Components".into(),
                count: 7
            }
        );
    }

    #[test]
    fn parse_properties_should_skip_unknown_type_and_continue_parsing() {
        let mut data = Vec::new();

        // Unknown: FloatVal (FloatProperty) = 4 bytes arbitrary
        data.extend(fname(8)); // name: "FloatVal"
        data.extend(fname(9)); // type: "FloatProperty" (unknown)
        data.extend(i64_le(4)); // size = 4
        data.extend(i32_le(0)); // array_index
        data.extend(i32_le(0x3dcc_cccd_u32 as i32)); // arbitrary float bytes (skipped)

        // IntProperty: NodeCount = 10
        data.extend(fname(1)); // name: "NodeCount"
        data.extend(fname(4)); // type: "IntProperty"
        data.extend(i64_le(4)); // size = 4
        data.extend(i32_le(0)); // array_index
        data.extend(i32_le(10)); // value

        // Terminator
        data.extend(fname(0)); // name: "None"

        let result = parse_properties(&data, 0, &name_table()).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0],
            ParsedProperty::Skipped {
                name: "FloatVal".into()
            }
        );
        assert_eq!(
            result[1],
            ParsedProperty::Int {
                name: "NodeCount".into(),
                value: 10
            }
        );
    }

    #[test]
    fn parse_properties_should_parse_object_property() {
        let mut data = Vec::new();

        // ObjectProperty: TargetObject → package index -1 (first import entry)
        data.extend(fname(10)); // name: "TargetObject"
        data.extend(fname(11)); // type: "ObjectProperty"
        data.extend(i64_le(4)); // size = 4
        data.extend(i32_le(0)); // array_index
        data.extend(i32_le(-1)); // value: FPackageIndex = -1

        // Terminator
        data.extend(fname(0)); // name: "None"

        let result = parse_properties(&data, 0, &name_table()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0],
            ParsedProperty::Object {
                name: "TargetObject".into(),
                class_name: "-1".into()
            }
        );
    }

    #[test]
    fn parse_properties_should_return_empty_vec_when_first_name_is_none() {
        let mut data = Vec::new();
        data.extend(fname(0)); // name: "None" immediately
        let result = parse_properties(&data, 0, &name_table()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn parse_properties_should_return_err_on_truncated_data() {
        // Only write the name FName for the first property, then cut off
        let mut data = Vec::new();
        data.extend(fname(1)); // name: "NodeCount" — but no type/size follows
        let result = parse_properties(&data, 0, &name_table());
        assert!(matches!(result, Err(ScanError::UnexpectedEof)));
    }

    #[test]
    fn parse_properties_should_respect_offset_parameter() {
        let mut data = vec![0u8; 16]; // 16 bytes of padding

        // IntProperty at offset 16: NodeCount = 99
        data.extend(fname(1)); // name: "NodeCount"
        data.extend(fname(4)); // type: "IntProperty"
        data.extend(i64_le(4)); // size = 4
        data.extend(i32_le(0)); // array_index
        data.extend(i32_le(99)); // value

        // Terminator
        data.extend(fname(0)); // name: "None"

        let result = parse_properties(&data, 16, &name_table()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0],
            ParsedProperty::Int {
                name: "NodeCount".into(),
                value: 99
            }
        );
    }
}
