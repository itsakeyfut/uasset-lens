use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read};

use super::map_io;
use crate::ScanError;

pub fn parse_name_table(data: &[u8], offset: u64, count: usize) -> Result<Vec<String>, ScanError> {
    let mut cur = Cursor::new(data);
    cur.set_position(offset);

    let mut names = Vec::with_capacity(count);
    for _ in 0..count {
        let len = cur.read_i32::<LittleEndian>().map_err(map_io)?;

        let name = if len == 0 {
            String::new()
        } else if len < 0 {
            // UTF-16LE: each char is 2 bytes (negative length encodes char count)
            let byte_count = (-len as u64) * 2;
            cur.set_position(cur.position() + byte_count);
            tracing::warn!("UTF-16 name entry skipped");
            String::new()
        } else {
            let mut bytes = vec![0u8; len as usize];
            cur.read_exact(&mut bytes).map_err(map_io)?;
            if bytes.last() == Some(&0) {
                bytes.pop();
            }
            // from_utf8 consumes the Vec directly — no copy in the happy path
            String::from_utf8(bytes).unwrap_or_else(|e| {
                tracing::warn!("invalid UTF-8 in name entry, using lossy decode");
                String::from_utf8_lossy(e.as_bytes()).into_owned()
            })
        };

        names.push(name);
        // Consume 4-byte hash suffix (u16 + u16)
        cur.read_u32::<LittleEndian>().map_err(map_io)?;
    }

    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::super::test_utils::*;
    use super::*;

    // Builds a minimal name table binary:
    // entries is &[&str] — each becomes a UTF-8 FString with a zero hash suffix.
    fn build_name_table(entries: &[&str]) -> Vec<u8> {
        let mut buf = Vec::new();
        for s in entries {
            let bytes = s.as_bytes();
            let len = (bytes.len() + 1) as i32; // +1 for null terminator
            buf.extend_from_slice(&len.to_le_bytes());
            buf.extend_from_slice(bytes);
            buf.push(0); // null terminator
            buf.extend_from_slice(&[0u8; 4]); // hash suffix
        }
        buf
    }

    #[test]
    fn parse_name_table_should_parse_inline_byte_sequence() {
        let data = build_name_table(&["None", "StaticMesh", "BP_Simple"]);
        let names = parse_name_table(&data, 0, 3).unwrap();
        assert_eq!(names, vec!["None", "StaticMesh", "BP_Simple"]);
    }

    #[test]
    fn parse_name_table_should_return_expected_names_for_bp_simple_fixture() {
        let data = read_fixture("valid/BP_Simple.uasset");
        let names = parse_name_table(&data, 537, 113).unwrap();
        assert_eq!(names.len(), 113);
        assert_eq!(names[0], "/Game/GameModes/BP_BaseGameMode");
        assert_eq!(names[1], "/Script/CoreUObject");
        assert_eq!(names[2], "/Script/Engine");
        assert_eq!(names[3], "AllNodes");
        assert_eq!(names[4], "ArrayProperty");
    }

    #[test]
    fn parse_name_table_should_handle_utf16_entry_as_empty_string_and_continue() {
        // UTF-16LE entry: len=-2 (2 chars = 4 data bytes), then a valid UTF-8 entry
        let mut data = Vec::new();
        // UTF-16 entry: len = -2, 4 bytes data ("AB" in UTF-16LE), 4-byte hash
        data.extend_from_slice(&(-2i32).to_le_bytes());
        data.extend_from_slice(&[0x41, 0x00, 0x42, 0x00]); // "AB" UTF-16LE
        data.extend_from_slice(&[0u8; 4]); // hash
        // Valid UTF-8 entry: "Good"
        data.extend_from_slice(&(5i32).to_le_bytes()); // len = 5 (4 chars + null)
        data.extend_from_slice(b"Good\0");
        data.extend_from_slice(&[0u8; 4]); // hash

        let names = parse_name_table(&data, 0, 2).unwrap();
        assert_eq!(names.len(), 2);
        assert_eq!(names[0], ""); // UTF-16 entry becomes empty
        assert_eq!(names[1], "Good");
    }
}
