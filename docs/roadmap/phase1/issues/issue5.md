# `crates/scanner` — ScanError and package header parser

## Summary

Define the `ScanError` error type and implement `parser/header.rs` to parse
`FPackageFileSummary` from a `.uasset` binary, extracting the version and table
offsets needed by subsequent parsers.
Complete when valid fixture headers parse successfully and `bad_magic.bin` returns
`ScanError::InvalidMagic`.

## Design Notes

**Magic number:** `0x9E2A83C1` — stored as little-endian bytes `C1 83 2A 9E` at offset 0.

**Relevant `FPackageFileSummary` fields (in parse order):**

```
Magic              u32      must equal 0x9E2A83C1
LegacyFileVersion  i32      must be -8 for UE5
LegacyUE3Version   i32      skip (always present)
FileVersionUE4     i32      skip for now
FileVersionUE5     i32      store in FPackageVersion
... (many fields to skip: custom versions, package name, flags, etc.)
NameCount          i32
NameOffset         i32
... (skip to import/export offsets)
ExportCount        i32
ExportOffset       i32
ImportCount        i32
ImportOffset       i32
```

Use `byteorder::ReadBytesExt` with `LittleEndian` for all reads. Never use `NativeEndian`.
Parse with a `std::io::Cursor<&[u8]>` so seeks map directly to file offsets.

> **Note**: The full header has ~40+ fields between FileVersionUE5 and NameCount. Use the
> UE5 source (`FPackageFileSummary::Serialize`) as reference for the exact field order.
> Fields that are not needed can be read and discarded rather than skipped by offset,
> as this is more robust against layout changes.

## Requirements

- [ ] Define `ScanError` enum: `InvalidMagic(u32)`, `UnsupportedVersion(i32, u32)`, `UnexpectedEof`, `Io(#[from] std::io::Error)` using `thiserror`
- [ ] Define `FPackageFileSummary` struct: `version: FPackageVersion`, `name_count: usize`, `name_offset: u64`, `import_count: usize`, `import_offset: u64`, `export_count: usize`, `export_offset: u64`
- [ ] Implement `parse_header(data: &[u8]) -> Result<FPackageFileSummary, ScanError>`
- [ ] Validate magic number, return `InvalidMagic(actual)` if wrong
- [ ] Reject `legacy_version != -8` with `UnsupportedVersion(legacy_version, file_version_ue5)`
- [ ] Map `std::io::ErrorKind::UnexpectedEof` to `ScanError::UnexpectedEof`
- [ ] Unit test: `bad_magic.bin` → `InvalidMagic`
- [ ] Unit test: `truncated.bin` → `UnexpectedEof`
- [ ] Unit test: each valid fixture header parses without error

## Related

- Depends on: #3 (FPackageVersion from shared), #4 (fixture files)
- Next: #6 — NameTable parser
- Docs: `docs/roadmap/phase1/ROADMAP.md` — Task 3-1, 3-2, `docs/rules/binary-parser.md`
