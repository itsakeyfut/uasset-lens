use byteorder::{LittleEndian, ReadBytesExt};
use shared::FPackageVersion;
use std::io::Cursor;

use super::map_io;
use crate::ScanError;

const UE_MAGIC: u32 = 0x9E2A83C1;

// FileVersionUE5 threshold at which SoftObjectPath list fields were added.
const VER_UE5_ADD_SOFTOBJECTPATH_LIST: i32 = 1008;

#[derive(Debug)]
pub struct FPackageFileSummary {
    pub version: FPackageVersion,
    pub name_count: usize,
    pub name_offset: u64,
    pub export_count: usize,
    pub export_offset: u64,
    pub import_count: usize,
    pub import_offset: u64,
    pub depends_offset: u64,
}

pub fn parse_header(data: &[u8]) -> Result<FPackageFileSummary, ScanError> {
    let mut cur = Cursor::new(data);

    let magic = cur.read_u32::<LittleEndian>().map_err(map_io)?;
    if magic != UE_MAGIC {
        return Err(ScanError::InvalidMagic(magic));
    }

    let legacy_version = cur.read_i32::<LittleEndian>().map_err(map_io)?;
    if legacy_version != -8 {
        return Err(ScanError::UnsupportedVersion(legacy_version, 0));
    }

    // LegacyUE3Version (always present when legacy_version < -4)
    let _legacy_ue3 = cur.read_i32::<LittleEndian>().map_err(map_io)?;

    let file_version_ue4 = cur.read_i32::<LittleEndian>().map_err(map_io)?;
    let file_version_ue5 = cur.read_i32::<LittleEndian>().map_err(map_io)?;

    let version = FPackageVersion {
        legacy_version,
        file_version_ue4: file_version_ue4 as u32,
        file_version_ue5: file_version_ue5 as u32,
    };

    if !version.is_ue5() {
        return Err(ScanError::UnsupportedVersion(
            legacy_version,
            file_version_ue5 as u32,
        ));
    }

    // FileVersionLicenseeUE
    let _licensee = cur.read_i32::<LittleEndian>().map_err(map_io)?;

    // CustomVersions: TArray — each entry is 16-byte GUID + 4-byte version
    let custom_count = cur.read_i32::<LittleEndian>().map_err(map_io)?;
    if custom_count < 0 {
        return Err(ScanError::UnexpectedEof);
    }
    let skip = (custom_count as u64).saturating_mul(20);
    cur.set_position(cur.position() + skip);

    // TotalHeaderSize
    let _total_header = cur.read_i32::<LittleEndian>().map_err(map_io)?;

    // FolderName (FString)
    skip_fstring(&mut cur)?;

    // PackageFlags
    let _pkg_flags = cur.read_u32::<LittleEndian>().map_err(map_io)?;

    // NameTable location
    let name_count = cur.read_i32::<LittleEndian>().map_err(map_io)?;
    if name_count < 0 {
        return Err(ScanError::InvalidData("negative name_count".into()));
    }
    let name_offset = cur.read_i32::<LittleEndian>().map_err(map_io)?;

    // SoftObjectPath list — added in UE5 version 1008
    if file_version_ue5 >= VER_UE5_ADD_SOFTOBJECTPATH_LIST {
        let _so_count = cur.read_i32::<LittleEndian>().map_err(map_io)?;
        let _so_offset = cur.read_i32::<LittleEndian>().map_err(map_io)?;
    }

    // LocalizationId (FString)
    skip_fstring(&mut cur)?;

    // GatherableTextData location (always present)
    let _gather_count = cur.read_i32::<LittleEndian>().map_err(map_io)?;
    let _gather_offset = cur.read_i32::<LittleEndian>().map_err(map_io)?;

    // Export and Import table locations
    let export_count = cur.read_i32::<LittleEndian>().map_err(map_io)?;
    if export_count < 0 {
        return Err(ScanError::InvalidData("negative export_count".into()));
    }
    let export_offset = cur.read_i32::<LittleEndian>().map_err(map_io)?;
    let import_count = cur.read_i32::<LittleEndian>().map_err(map_io)?;
    if import_count < 0 {
        return Err(ScanError::InvalidData("negative import_count".into()));
    }
    let import_offset = cur.read_i32::<LittleEndian>().map_err(map_io)?;

    let depends_offset = cur.read_i32::<LittleEndian>().map_err(map_io)?;

    Ok(FPackageFileSummary {
        version,
        name_count: name_count as usize,
        name_offset: name_offset as u64,
        export_count: export_count as usize,
        export_offset: export_offset as u64,
        import_count: import_count as usize,
        import_offset: import_offset as u64,
        depends_offset: depends_offset as u64,
    })
}

fn skip_fstring(cur: &mut Cursor<&[u8]>) -> Result<(), ScanError> {
    let len = cur.read_i32::<LittleEndian>().map_err(map_io)?;
    let byte_count: u64 = if len == 0 {
        0
    } else if len < 0 {
        // UTF-16LE: each char is 2 bytes (negative length encodes char count)
        (-len as u64) * 2
    } else {
        // UTF-8/ASCII including null terminator
        len as u64
    };
    cur.set_position(cur.position() + byte_count);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::test_utils::*;
    use super::*;

    #[test]
    fn parse_header_should_return_invalid_magic_for_bad_magic_bin() {
        let data = read_fixture("invalid/bad_magic.bin");
        assert!(matches!(
            parse_header(&data),
            Err(ScanError::InvalidMagic(0x00000000))
        ));
    }

    #[test]
    fn parse_header_should_return_unexpected_eof_for_truncated_bin() {
        let data = read_fixture("invalid/truncated.bin");
        assert!(matches!(parse_header(&data), Err(ScanError::UnexpectedEof)));
    }

    #[test]
    fn parse_header_should_succeed_for_bp_simple_fixture() {
        let data = read_fixture("valid/BP_Simple.uasset");
        let header = parse_header(&data).unwrap();
        assert!(header.version.is_ue5());
        // Validated against raw binary: offset 272 in BP_Simple.uasset
        assert_eq!(header.name_count, 113);
        assert_eq!(header.name_offset, 537);
        assert_eq!(header.export_count, 12);
        assert_eq!(header.export_offset, 4133);
        assert_eq!(header.import_count, 22);
        assert_eq!(header.import_offset, 3253);
        assert_eq!(header.depends_offset, 5477);
    }

    #[test]
    fn parse_header_should_succeed_for_t_rock_fixture() {
        let data = read_fixture("valid/T_Rock.uasset");
        let header = parse_header(&data).unwrap();
        assert!(header.version.is_ue5());
        assert!(header.name_offset < data.len() as u64);
        assert!(header.import_offset < data.len() as u64);
        assert!(header.export_offset < data.len() as u64);
    }

    #[test]
    fn parse_header_should_succeed_for_sm_cube_fixture() {
        let data = read_fixture("valid/SM_Cube.uasset");
        let header = parse_header(&data).unwrap();
        assert!(header.version.is_ue5());
        assert!(header.name_offset < data.len() as u64);
        assert!(header.import_offset < data.len() as u64);
        assert!(header.export_offset < data.len() as u64);
    }

    #[test]
    fn parse_header_should_succeed_for_m_basic_fixture() {
        let data = read_fixture("valid/M_Basic.uasset");
        let header = parse_header(&data).unwrap();
        assert!(header.version.is_ue5());
        assert!(header.name_offset < data.len() as u64);
        assert!(header.import_offset < data.len() as u64);
        assert!(header.export_offset < data.len() as u64);
    }

    #[test]
    fn parse_header_should_succeed_for_redirect_fixture() {
        let data = read_fixture("valid/Redirect.uasset");
        let header = parse_header(&data).unwrap();
        assert!(header.version.is_ue5());
        assert!(header.name_offset < data.len() as u64);
        assert!(header.import_offset < data.len() as u64);
        assert!(header.export_offset < data.len() as u64);
    }

    #[test]
    fn parse_header_should_succeed_for_l_test_map_fixture() {
        let data = read_fixture("valid/L_TestMap.umap");
        let header = parse_header(&data).unwrap();
        assert!(header.version.is_ue5());
        assert!(header.name_offset < data.len() as u64);
        assert!(header.import_offset < data.len() as u64);
        assert!(header.export_offset < data.len() as u64);
    }

    #[test]
    fn parse_header_should_return_invalid_data_when_name_count_is_negative() {
        let mut data = read_fixture("valid/BP_Simple.uasset");
        // name_count is the i32 at byte offset 272 (see parse_header_should_succeed_for_bp_simple_fixture)
        data[272..276].copy_from_slice(&(-1i32).to_le_bytes());
        assert!(matches!(
            parse_header(&data),
            Err(ScanError::InvalidData(_))
        ));
    }

    #[test]
    fn parse_header_should_return_invalid_data_when_export_count_is_negative() {
        let mut data = read_fixture("valid/BP_Simple.uasset");
        // export_count is at byte offset 333; identified by the unique 20-byte sequence
        // [export_count=12, export_offset=4133, import_count=22, import_offset=3253, depends_offset=5477]
        data[333..337].copy_from_slice(&(-1i32).to_le_bytes());
        assert!(matches!(
            parse_header(&data),
            Err(ScanError::InvalidData(_))
        ));
    }
}
