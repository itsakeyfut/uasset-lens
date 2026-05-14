pub mod export;
pub mod header;
pub mod import;
pub mod name_table;
pub mod properties;

#[cfg(test)]
pub mod test_utils;

use crate::ScanError;

fn map_io(e: std::io::Error) -> ScanError {
    if e.kind() == std::io::ErrorKind::UnexpectedEof {
        ScanError::UnexpectedEof
    } else {
        ScanError::Io(e)
    }
}
