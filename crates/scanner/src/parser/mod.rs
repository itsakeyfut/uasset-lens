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

fn map_io(e: std::io::Error) -> ScanError {
    if e.kind() == std::io::ErrorKind::UnexpectedEof {
        ScanError::UnexpectedEof
    } else {
        ScanError::Io(e)
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

// Keeps FString skip logic in one place so header and soft_object_paths stay in sync.
fn skip_fstring(cur: &mut Cursor<&[u8]>) -> Result<(), ScanError> {
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
